//! Play on other speakers: DLNA/UPnP renderers, Chromecast, and AirPlay receivers on the
//! local network. Needle plays as usual and sends what it plays as a live stream, so the
//! queue, crossfade, equalizer, and stems all work on the speaker too.
pub mod airplay;
pub mod chromecast;
pub mod discover;
pub mod dlna;
pub mod http;
pub mod output;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, UdpSocket};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Dlna,
    Chromecast,
    AirPlay,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Dlna => "DLNA",
            Kind::Chromecast => "Chromecast",
            Kind::AirPlay => "AirPlay",
        }
    }
}

/// A speaker found on the network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Speaker {
    pub kind: Kind,
    pub name: String,
    /// DLNA: the device description URL. Chromecast and AirPlay: `ip:port`.
    pub address: String,
}

/// Output device names that mean a speaker start with this.
pub const PREFIX: &str = "speaker:";

impl Speaker {
    /// The speaker as an output device name, for the settings.
    pub fn device_name(&self) -> String {
        format!(
            "{PREFIX}{}",
            serde_json::to_string(self).unwrap_or_default()
        )
    }
    pub fn from_device_name(name: &str) -> Option<Self> {
        serde_json::from_str(name.strip_prefix(PREFIX)?).ok()
    }
    /// The speaker's IP address and port.
    pub fn socket(&self) -> Option<SocketAddr> {
        let host = match self.kind {
            Kind::Dlna => {
                let rest = self.address.split("://").nth(1)?;
                rest.split('/').next()?.to_string()
            }
            _ => self.address.clone(),
        };
        host.parse().ok().or_else(|| {
            let (ip, port) = host.rsplit_once(':')?;
            Some(SocketAddr::new(ip.parse().ok()?, port.parse().ok()?))
        })
    }
}

/// How the speaker's display should describe what is playing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// A URL the speaker can fetch the cover from.
    pub cover: Option<String>,
}

/// The address of this computer that `peer` can reach.
pub fn local_ip_for(peer: SocketAddr) -> Result<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.connect(peer)?;
    Ok(socket.local_addr()?.ip())
}

/// Controls a speaker that plays a stream from a URL.
pub trait Remote: Send {
    /// Start playing the stream at `url`.
    fn load(&mut self, url: &str, meta: &Meta) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    /// Keep the connection alive and read what the speaker reports; called about once a
    /// second.
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Connect to a speaker that plays URLs (DLNA and Chromecast).
pub fn connect(speaker: &Speaker) -> Result<Box<dyn Remote>> {
    Ok(match speaker.kind {
        Kind::Dlna => Box::new(dlna::Renderer::connect(&speaker.address)?),
        Kind::Chromecast => Box::new(chromecast::Device::connect(&speaker.address)?),
        Kind::AirPlay => anyhow::bail!("AirPlay speakers take audio directly, not a URL"),
    })
}

/// Escape text for XML.
pub(crate) fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speakers_round_trip_through_device_names() {
        let s = Speaker {
            kind: Kind::Dlna,
            name: "Living room".into(),
            address: "http://192.168.1.20:49152/desc.xml".into(),
        };
        assert_eq!(Speaker::from_device_name(&s.device_name()), Some(s.clone()));
        assert_eq!(s.socket(), Some("192.168.1.20:49152".parse().unwrap()));
        let c = Speaker {
            kind: Kind::Chromecast,
            name: "TV".into(),
            address: "192.168.1.30:8009".into(),
        };
        assert_eq!(c.socket(), Some("192.168.1.30:8009".parse().unwrap()));
        assert_eq!(Speaker::from_device_name("Speakers (Realtek)"), None);
        assert_eq!(xml_escape("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
    }

    /// A pretend DLNA speaker that really downloads the stream it is given.
    fn listening_renderer() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let heard = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let counter = heard.clone();
        std::thread::spawn(move || {
            for mut connection in listener.incoming().flatten() {
                let mut reader = BufReader::new(connection.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = header.to_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = String::from_utf8_lossy(&body).to_string();
                let reply = if line.starts_with("GET") {
                    super::dlna::tests::DESCRIPTION.to_string()
                } else {
                    String::new()
                };
                if let Some(start) = body.find("<CurrentURI>") {
                    let url = body[start + 12..body.find("</CurrentURI>").unwrap()].to_string();
                    let counter = counter.clone();
                    std::thread::spawn(move || {
                        let mut response = reqwest::blocking::get(url).unwrap();
                        let mut chunk = [0u8; 8192];
                        while let Ok(n) = response.read(&mut chunk) {
                            if n == 0 {
                                break;
                            }
                            counter.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                        }
                    });
                }
                write!(
                    connection,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
        });
        (format!("http://127.0.0.1:{port}/desc.xml"), heard)
    }

    #[test]
    fn the_player_plays_through_a_network_speaker() {
        use crate::{
            audio::{Command, Player, QueueItem},
            database::Library,
            model::{Settings, Track},
        };
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("song.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&song, spec).unwrap();
        for i in 0..44_100 * 4 {
            let v = ((i as f32 * 440. * std::f32::consts::TAU / 44_100.).sin() * 8000.) as i16;
            writer.write_sample(v).unwrap();
            writer.write_sample(v).unwrap();
        }
        writer.finalize().unwrap();
        let library = Library::open(dir.path().join("data")).unwrap();
        let track = Track {
            id: "t".into(),
            path: song.to_string_lossy().into(),
            title: "Tone".into(),
            duration: 4.,
            format: "WAV".into(),
            ..Default::default()
        };
        library.upsert(&track).unwrap();
        let (description, heard) = listening_renderer();
        let speaker = Speaker {
            kind: Kind::Dlna,
            name: "Kitchen".into(),
            address: description,
        };
        let settings = Settings {
            output_device: Some(speaker.device_name()),
            ..Default::default()
        };
        library.save_settings(&settings).unwrap();
        let player = Player::new(library);
        player.send(Command::Play(vec![QueueItem {
            track,
            reason: String::new(),
        }]));
        let start = std::time::Instant::now();
        // A second of CD audio is 176,400 bytes.
        while heard.load(std::sync::atomic::Ordering::Relaxed) < 176_400
            && start.elapsed().as_secs() < 8
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let state = player.state();
        player.shutdown();
        assert!(
            heard.load(std::sync::atomic::Ordering::Relaxed) >= 176_400,
            "the speaker got {} bytes",
            heard.load(std::sync::atomic::Ordering::Relaxed)
        );
        assert_eq!(state.output, "Kitchen");
        assert!(state.playing);
    }
}
