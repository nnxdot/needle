//! A small web server on this computer that speakers fetch from: the live stream of what
//! Needle plays, as WAV, and album covers.
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

/// Audio waiting for the speaker to read it.
pub struct Stream {
    buffer: Mutex<Buffer>,
    ready: Condvar,
    pub rate: u32,
    pub channels: u16,
    /// Most bytes kept when nobody reads (older ones are dropped).
    limit: usize,
}

struct Buffer {
    data: VecDeque<u8>,
    closed: bool,
    /// Bumped when a new reader connects; older readers stop.
    reader: u64,
    /// Bytes handed to readers so far.
    sent: u64,
}

impl Stream {
    fn new(rate: u32, channels: u16) -> Self {
        Self {
            buffer: Mutex::new(Buffer {
                data: VecDeque::new(),
                closed: false,
                reader: 0,
                sent: 0,
            }),
            ready: Condvar::new(),
            rate,
            channels,
            // Twenty seconds of audio.
            limit: rate as usize * channels as usize * 2 * 20,
        }
    }
    /// Add 16-bit little-endian samples.
    pub fn push(&self, bytes: &[u8]) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.data.extend(bytes);
        let excess = buffer.data.len().saturating_sub(self.limit);
        if excess > 0 {
            // Keep whole frames.
            let frame = self.channels as usize * 2;
            buffer.data.drain(..excess.div_ceil(frame) * frame);
        }
        self.ready.notify_all();
    }
    pub fn close(&self) {
        self.buffer.lock().unwrap().closed = true;
        self.ready.notify_all();
    }
    /// Seconds of audio sent to the speaker so far.
    pub fn sent_seconds(&self) -> f64 {
        let sent = self.buffer.lock().unwrap().sent;
        sent as f64 / (self.rate as f64 * self.channels as f64 * 2.)
    }
    /// Whether a speaker has connected to read.
    pub fn connected(&self) -> bool {
        self.buffer.lock().unwrap().reader > 0
    }
}

/// A WAV header for a stream of unknown length.
pub fn wav_header(rate: u32, channels: u16) -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    let block = channels as u32 * 2;
    h.extend(b"RIFF");
    h.extend(u32::MAX.to_le_bytes());
    h.extend(b"WAVEfmt ");
    h.extend(16u32.to_le_bytes());
    h.extend(1u16.to_le_bytes());
    h.extend(channels.to_le_bytes());
    h.extend(rate.to_le_bytes());
    h.extend((rate * block).to_le_bytes());
    h.extend((block as u16).to_le_bytes());
    h.extend(16u16.to_le_bytes());
    h.extend(b"data");
    h.extend((u32::MAX - 36).to_le_bytes());
    h
}

pub struct Server {
    pub port: u16,
    streams: Mutex<HashMap<String, Arc<Stream>>>,
    covers: Mutex<HashMap<String, PathBuf>>,
}

static SERVER: OnceLock<Result<Arc<Server>, String>> = OnceLock::new();

/// The server, started on first use.
pub fn server() -> Result<Arc<Server>> {
    SERVER
        .get_or_init(|| start().map_err(|e| format!("{e:#}")))
        .clone()
        .map_err(anyhow::Error::msg)
}

fn start() -> Result<Arc<Server>> {
    let listener =
        TcpListener::bind(("0.0.0.0", 0)).context("Could not start the speaker stream server")?;
    let server = Arc::new(Server {
        port: listener.local_addr()?.port(),
        streams: Mutex::default(),
        covers: Mutex::default(),
    });
    let shared = server.clone();
    std::thread::Builder::new()
        .name("needle-cast-http".into())
        .spawn(move || {
            for connection in listener.incoming().flatten() {
                let server = shared.clone();
                std::thread::spawn(move || {
                    let _ = server.serve(connection);
                });
            }
        })?;
    Ok(server)
}

impl Server {
    /// A new live stream, at `/live/<id>.wav`.
    pub fn open_stream(&self, id: &str, rate: u32, channels: u16) -> Arc<Stream> {
        let stream = Arc::new(Stream::new(rate, channels));
        self.streams
            .lock()
            .unwrap()
            .insert(id.to_string(), stream.clone());
        stream
    }
    pub fn close_stream(&self, id: &str) {
        if let Some(stream) = self.streams.lock().unwrap().remove(id) {
            stream.close();
        }
    }
    /// Offer a cover image; returns its path on the server.
    pub fn share_cover(&self, path: PathBuf) -> String {
        let token = format!("{:x}", md5::compute(path.to_string_lossy().as_bytes()));
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "jpg".into());
        self.covers.lock().unwrap().insert(token.clone(), path);
        format!("/cover/{token}.{extension}")
    }

    fn serve(&self, mut connection: TcpStream) -> Result<()> {
        connection.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut reader = BufReader::new(connection.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let mut parts = line.split_whitespace();
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        // Read and ignore the headers.
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
                break;
            }
        }
        let head = method.eq_ignore_ascii_case("HEAD");
        let path = target.split('?').next().unwrap_or("");
        if let Some(name) = path.strip_prefix("/live/") {
            let id = name.trim_end_matches(".wav");
            let stream = self.streams.lock().unwrap().get(id).cloned();
            let Some(stream) = stream else {
                return not_found(&mut connection);
            };
            write!(
                connection,
                "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nConnection: close\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\ntransferMode.dlna.org: Streaming\r\ncontentFeatures.dlna.org: DLNA.ORG_PN=WAV;DLNA.ORG_OP=00;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000\r\n\r\n"
            )?;
            if head {
                return Ok(());
            }
            connection.write_all(&wav_header(stream.rate, stream.channels))?;
            return stream_to(&stream, connection);
        }
        if let Some(name) = path.strip_prefix("/cover/") {
            let token = name.split('.').next().unwrap_or("");
            let file = self.covers.lock().unwrap().get(token).cloned();
            let Some(bytes) = file.and_then(|f| std::fs::read(f).ok()) else {
                return not_found(&mut connection);
            };
            let kind = if name.ends_with(".png") {
                "image/png"
            } else {
                "image/jpeg"
            };
            write!(
                connection,
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                bytes.len()
            )?;
            if !head {
                connection.write_all(&bytes)?;
            }
            return Ok(());
        }
        not_found(&mut connection)
    }
}

fn not_found(connection: &mut TcpStream) -> Result<()> {
    write!(
        connection,
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    Ok(())
}

/// Hand the stream to one reader until it goes away, the stream closes, or another reader
/// takes over.
fn stream_to(stream: &Stream, mut connection: TcpStream) -> Result<()> {
    connection.set_nodelay(true)?;
    let me = {
        let mut buffer = stream.buffer.lock().unwrap();
        buffer.reader += 1;
        buffer.reader
    };
    stream.ready.notify_all();
    loop {
        let chunk: Vec<u8> = {
            let mut buffer = stream.buffer.lock().unwrap();
            while buffer.data.is_empty() && !buffer.closed && buffer.reader == me {
                buffer = stream
                    .ready
                    .wait_timeout(buffer, Duration::from_millis(500))
                    .unwrap()
                    .0;
            }
            if buffer.closed || buffer.reader != me {
                return Ok(());
            }
            let n = buffer.data.len().min(64 * 1024);
            buffer.sent += n as u64;
            buffer.data.drain(..n).collect()
        };
        connection.write_all(&chunk)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn serves_a_live_wav_stream_and_covers() {
        let server = server().unwrap();
        let stream = server.open_stream("test", 44_100, 2);
        stream.push(&[1, 0, 2, 0, 3, 0, 4, 0]);
        let dir = tempfile::tempdir().unwrap();
        let cover = dir.path().join("c.png");
        std::fs::write(&cover, b"png!").unwrap();
        let cover_path = server.share_cover(cover);

        let mut client = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        write!(client, "GET /live/test.wav HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut got = vec![0u8; 512];
        let mut total = 0;
        while total < 44 + 8 {
            let n = client.read(&mut got[total..]).unwrap();
            assert!(n > 0);
            total += n;
            let text = String::from_utf8_lossy(&got[..total]);
            if let Some(end) = text.find("\r\n\r\n") {
                assert!(text.starts_with("HTTP/1.1 200"));
                assert!(text.contains("audio/wav"));
                let body = &got[end + 4..total];
                if body.len() >= 52 {
                    assert_eq!(&body[..4], b"RIFF");
                    assert_eq!(&body[44..52], &[1, 0, 2, 0, 3, 0, 4, 0]);
                    break;
                }
            }
        }
        server.close_stream("test");

        let mut client = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        write!(client, "GET {cover_path} HTTP/1.1\r\n\r\n").unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.contains("image/png") && reply.ends_with("png!"));

        let mut client = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        write!(client, "GET /live/nope.wav HTTP/1.1\r\n\r\n").unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 404"));
    }
}

#[cfg(test)]
mod browser {
    /// Serve a live tone for a minute, for checking playback in a browser:
    /// `cargo test serve_a_tone -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn serve_a_tone() {
        let server = super::server().unwrap();
        let stream = server.open_stream("tone", 44_100, 2);
        println!("URL http://127.0.0.1:{}/live/tone.wav", server.port);
        let start = std::time::Instant::now();
        let mut frame = 0u64;
        while start.elapsed().as_secs() < 90 {
            let target = (start.elapsed().as_secs_f64() * 44_100.) as u64 + 22_050;
            let mut bytes = vec![];
            while frame < target {
                let v =
                    ((frame as f64 * 440. * std::f64::consts::TAU / 44_100.).sin() * 6000.) as i16;
                bytes.extend(v.to_le_bytes());
                bytes.extend(v.to_le_bytes());
                frame += 1;
            }
            stream.push(&bytes);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
