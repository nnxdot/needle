//! Chromecast: a TLS connection to port 8009 carrying small protobuf "CastMessage" frames with
//! JSON inside. Needle starts the Default Media Receiver and asks it to play the live stream.
use super::{Meta, Remote};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};

const CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const MEDIA: &str = "urn:x-cast:com.google.cast.media";
/// Google's Default Media Receiver.
const APP: &str = "CC1AD845";
const SENDER: &str = "sender-needle";

// ---------------------------------------------------------------- CastMessage framing

fn varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn text_field(number: u8, text: &str, out: &mut Vec<u8>) {
    out.push(number << 3 | 2);
    varint(text.len() as u64, out);
    out.extend(text.as_bytes());
}

/// One CastMessage, with its four-byte length in front.
pub(crate) fn frame(source: &str, destination: &str, namespace: &str, payload: &str) -> Vec<u8> {
    let mut message = vec![0x08, 0x00]; // protocol_version = CASTV2_1_0
    text_field(2, source, &mut message);
    text_field(3, destination, &mut message);
    text_field(4, namespace, &mut message);
    message.extend([0x28, 0x00]); // payload_type = STRING
    text_field(6, payload, &mut message);
    let mut out = (message.len() as u32).to_be_bytes().to_vec();
    out.extend(message);
    out
}

/// A received message: source, destination, namespace, and text payload.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Message {
    pub source: String,
    pub destination: String,
    pub namespace: String,
    pub payload: String,
}

pub(crate) fn parse(mut bytes: &[u8]) -> Result<Message> {
    let read_varint = |bytes: &mut &[u8]| -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = bytes.split_first().context("Truncated message")?;
            *bytes = rest;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        bail!("Bad varint")
    };
    let mut message = Message::default();
    while !bytes.is_empty() {
        let key = read_varint(&mut bytes)?;
        match key & 7 {
            0 => {
                read_varint(&mut bytes)?;
            }
            2 => {
                let length = read_varint(&mut bytes)? as usize;
                if length > bytes.len() {
                    bail!("Truncated message");
                }
                let (value, rest) = bytes.split_at(length);
                bytes = rest;
                let text = String::from_utf8_lossy(value).to_string();
                match key >> 3 {
                    2 => message.source = text,
                    3 => message.destination = text,
                    4 => message.namespace = text,
                    6 => message.payload = text,
                    _ => {}
                }
            }
            _ => bail!("Unexpected field type"),
        }
    }
    Ok(message)
}

// ---------------------------------------------------------------- TLS

/// Chromecasts present certificates signed by Google's device CA, which no public root
/// store has; the connection is only on the local network, so any certificate is accepted.
#[derive(Debug)]
struct AnyCertificate(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

type Tls = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

fn tls(address: SocketAddr) -> Result<Tls> {
    let socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .context("The Chromecast did not answer")?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    socket.set_nodelay(true)?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::IpAddress(address.ip().into());
    let connection = rustls::ClientConnection::new(Arc::new(config), name)?;
    Ok(rustls::StreamOwned::new(connection, socket))
}

// ---------------------------------------------------------------- the device

pub struct Device<S: Read + Write + Send = Tls> {
    stream: S,
    inbox: Vec<u8>,
    request: u64,
    /// The media receiver app's transport and session, once launched.
    app: Option<(String, String)>,
    media_session: Option<i64>,
    last_ping: Instant,
}

impl Device<Tls> {
    pub fn connect(address: &str) -> Result<Self> {
        let address: SocketAddr = address.parse().context("Bad Chromecast address")?;
        Device::over(tls(address)?)
    }
}

impl<S: Read + Write + Send> Device<S> {
    pub(crate) fn over(stream: S) -> Result<Self> {
        let mut device = Self {
            stream,
            inbox: vec![],
            request: 0,
            app: None,
            media_session: None,
            last_ping: Instant::now(),
        };
        device.send("receiver-0", CONNECTION, json!({"type": "CONNECT"}))?;
        Ok(device)
    }

    fn send(&mut self, destination: &str, namespace: &str, mut payload: Value) -> Result<u64> {
        self.request += 1;
        if namespace == RECEIVER || namespace == MEDIA {
            payload["requestId"] = json!(self.request);
        }
        self.stream
            .write_all(&frame(SENDER, destination, namespace, &payload.to_string()))?;
        self.stream.flush()?;
        Ok(self.request)
    }

    /// Read what has arrived (waiting at most a moment), answering pings.
    fn receive(&mut self) -> Result<Vec<Value>> {
        let mut chunk = [0u8; 16 * 1024];
        match self.stream.read(&mut chunk) {
            Ok(0) => bail!("The Chromecast closed the connection"),
            Ok(n) => self.inbox.extend(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.into()),
        }
        let mut out = vec![];
        while self.inbox.len() >= 4 {
            let length = u32::from_be_bytes(self.inbox[..4].try_into().unwrap()) as usize;
            if self.inbox.len() < 4 + length {
                break;
            }
            let message = parse(&self.inbox[4..4 + length])?;
            self.inbox.drain(..4 + length);
            let payload: Value = serde_json::from_str(&message.payload).unwrap_or(Value::Null);
            if message.namespace == HEARTBEAT && payload["type"] == "PING" {
                self.send(&message.source, HEARTBEAT, json!({"type": "PONG"}))?;
                continue;
            }
            if payload["type"] == "MEDIA_STATUS"
                && let Some(id) = payload["status"][0]["mediaSessionId"].as_i64()
            {
                self.media_session = Some(id);
            }
            out.push(payload);
        }
        Ok(out)
    }

    /// Wait for a reply that `matches`, up to `timeout`.
    fn wait(&mut self, timeout: Duration, matches: impl Fn(&Value) -> bool) -> Result<Value> {
        let end = Instant::now() + timeout;
        while Instant::now() < end {
            for reply in self.receive()? {
                if matches!(
                    reply["type"].as_str(),
                    Some("LOAD_FAILED" | "LAUNCH_ERROR" | "INVALID_REQUEST")
                ) {
                    bail!(
                        "The Chromecast said no: {}",
                        reply["reason"]
                            .as_str()
                            .or(reply["type"].as_str())
                            .unwrap_or("?")
                    );
                }
                if matches(&reply) {
                    return Ok(reply);
                }
            }
        }
        bail!("The Chromecast did not answer in time")
    }

    fn launch(&mut self) -> Result<(String, String)> {
        if let Some(app) = &self.app {
            return Ok(app.clone());
        }
        self.send(
            "receiver-0",
            RECEIVER,
            json!({"type": "LAUNCH", "appId": APP}),
        )?;
        let status = self.wait(Duration::from_secs(15), |r| {
            r["type"] == "RECEIVER_STATUS"
                && r["status"]["applications"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|app| app["appId"] == APP))
        })?;
        let app = status["status"]["applications"]
            .as_array()
            .and_then(|a| a.iter().find(|app| app["appId"] == APP))
            .context("No media receiver")?;
        let transport = app["transportId"]
            .as_str()
            .context("No transport")?
            .to_string();
        let session = app["sessionId"].as_str().unwrap_or_default().to_string();
        self.send(&transport, CONNECTION, json!({"type": "CONNECT"}))?;
        self.app = Some((transport.clone(), session.clone()));
        Ok((transport, session))
    }

    fn media(&mut self, kind: &str) -> Result<()> {
        let (Some((transport, _)), Some(id)) = (self.app.clone(), self.media_session) else {
            return Ok(());
        };
        self.send(
            &transport,
            MEDIA,
            json!({"type": kind, "mediaSessionId": id}),
        )?;
        Ok(())
    }
}

impl<S: Read + Write + Send> Remote for Device<S> {
    fn load(&mut self, url: &str, meta: &Meta) -> Result<()> {
        let (transport, session) = self.launch()?;
        let mut metadata = json!({
            "metadataType": 3,
            "title": meta.title,
            "artist": meta.artist,
            "albumName": meta.album,
        });
        if let Some(cover) = &meta.cover {
            metadata["images"] = json!([{"url": cover}]);
        }
        self.media_session = None;
        self.send(
            &transport,
            MEDIA,
            json!({
                "type": "LOAD",
                "sessionId": session,
                "autoplay": true,
                "media": {"contentId": url, "contentType": "audio/wav", "streamType": "LIVE", "metadata": metadata},
            }),
        )?;
        self.wait(Duration::from_secs(20), |r| {
            r["type"] == "MEDIA_STATUS" && r["status"][0]["mediaSessionId"].is_i64()
        })?;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.media("PAUSE")
    }
    fn resume(&mut self) -> Result<()> {
        self.media("PLAY")
    }
    fn stop(&mut self) -> Result<()> {
        if let Some((_, session)) = self.app.take() {
            self.send(
                "receiver-0",
                RECEIVER,
                json!({"type": "STOP", "sessionId": session}),
            )?;
        }
        Ok(())
    }
    fn tick(&mut self) -> Result<()> {
        if self.last_ping.elapsed() > Duration::from_secs(5) {
            self.last_ping = Instant::now();
            self.send("receiver-0", HEARTBEAT, json!({"type": "PING"}))?;
        }
        self.receive().map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn frames_round_trip() {
        let bytes = frame("a", "receiver-0", CONNECTION, "{\"type\":\"CONNECT\"}");
        let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(length, bytes.len() - 4);
        let message = parse(&bytes[4..]).unwrap();
        assert_eq!(message.source, "a");
        assert_eq!(message.destination, "receiver-0");
        assert_eq!(message.namespace, CONNECTION);
        assert_eq!(message.payload, "{\"type\":\"CONNECT\"}");
        // Long payloads need multi-byte lengths.
        let long = "x".repeat(300);
        assert_eq!(
            parse(&frame("s", "d", "n", &long)[4..]).unwrap().payload,
            long
        );
    }

    /// A pretend Chromecast (without TLS) that launches the app and loads media.
    fn fake() -> (SocketAddr, std::sync::mpsc::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut inbox: Vec<u8> = vec![];
            let mut chunk = [0u8; 4096];
            loop {
                let n = match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                inbox.extend(&chunk[..n]);
                while inbox.len() >= 4 {
                    let length = u32::from_be_bytes(inbox[..4].try_into().unwrap()) as usize;
                    if inbox.len() < 4 + length {
                        break;
                    }
                    let message = parse(&inbox[4..4 + length]).unwrap();
                    inbox.drain(..4 + length);
                    let payload: Value = serde_json::from_str(&message.payload).unwrap();
                    tx.send(payload.clone()).unwrap();
                    let reply = |ns: &str, value: Value| {
                        frame(
                            &message.destination,
                            &message.source,
                            ns,
                            &value.to_string(),
                        )
                    };
                    let out = match payload["type"].as_str() {
                        Some("LAUNCH") => {
                            // A ping first, which the sender must answer.
                            let mut out = reply(HEARTBEAT, json!({"type": "PING"}));
                            out.extend(reply(RECEIVER, json!({"type": "RECEIVER_STATUS", "status": {"applications": [{"appId": APP, "transportId": "web-7", "sessionId": "s-1"}]}})));
                            out
                        }
                        Some("LOAD") => reply(
                            MEDIA,
                            json!({"type": "MEDIA_STATUS", "status": [{"mediaSessionId": 42, "playerState": "BUFFERING"}]}),
                        ),
                        _ => vec![],
                    };
                    socket.write_all(&out).unwrap();
                }
            }
        });
        (address, rx)
    }

    #[test]
    fn launches_loads_pauses_and_stops() {
        let (address, seen) = fake();
        let socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let mut device = Device::over(socket).unwrap();
        let meta = Meta {
            title: "Song".into(),
            artist: "Band".into(),
            album: "LP".into(),
            cover: Some("http://h/c.jpg".into()),
        };
        device
            .load("http://10.0.0.2:5000/live/1.wav", &meta)
            .unwrap();
        device.pause().unwrap();
        device.resume().unwrap();
        device.stop().unwrap();
        let mut types = vec![];
        let mut load = Value::Null;
        while let Ok(message) = seen.recv_timeout(Duration::from_millis(500)) {
            if message["type"] == "LOAD" {
                load = message.clone();
            }
            types.push(message["type"].as_str().unwrap_or("").to_string());
        }
        assert_eq!(
            types,
            vec![
                "CONNECT", "LAUNCH", "PONG", "CONNECT", "LOAD", "PAUSE", "PLAY", "STOP"
            ]
        );
        assert_eq!(
            load["media"]["contentId"],
            "http://10.0.0.2:5000/live/1.wav"
        );
        assert_eq!(load["media"]["metadata"]["title"], "Song");
        assert_eq!(load["sessionId"], "s-1");
    }
}
