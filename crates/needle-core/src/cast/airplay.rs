//! AirPlay (the RAOP protocol of AirPlay 1, which AirPort Express, Apple TVs, many AirPlay
//! speakers, and shairport-sync accept). RTSP over TCP sets up a session; audio goes as RTP
//! over UDP in uncompressed ALAC frames of 352 samples, optionally AES-encrypted; the receiver
//! asks for the time on a timing port, and gets sync packets on a control port.
//! AirPlay 2's pairing is not supported.
use super::{Meta, Speaker, output::Destination};
use aes::cipher::{BlockEncryptMut, KeyIvInit, generic_array::GenericArray};
use anyhow::{Context, Result, bail};
use base64::Engine;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const FRAMES: usize = 352;
/// The receiver plays this many frames (two seconds) behind the timestamps it is sent.
const LATENCY: u32 = 88_200;
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD_NO_PAD;

/// Apple's RAOP public key (the AirPort Express key every AirPlay 1 receiver accepts).
const APPLE_KEY: &str = "59dE8qLieItsH1WgjrcFRKj6eUWqi+bGLOX1HL3U3GhC/j0Qg90u3sG/1CUtwC5vOYvfDmFI6oSFXi5ELabWJmT2dKHzBJKa3k9ok+8t9ucRqMd6DZHJ2YCCLlDRKSKv6kDqnw4UwPdpOMXziC/AMj3Z/lUVX1G7WSHCAWKf1zNS1eLvqr+boEjXuBOitnZ/bDzPHrTOZz0Dew0uowxf/+sG+NCK3eQJVxqcaJ/vEHKIVd2M+5qL71yJQ+87X6oV3eaYvt3zWZYD6z5vYTcrtij2VZ9Zmni/UAaHqn9JdsBWLUEpVviYnhimNVvYFZeCXg/IdTQ+x4IRdiXNv5hEew";

// ---------------------------------------------------------------- ALAC and RTP

/// 352 stereo frames as an uncompressed ALAC frame.
pub(crate) fn alac(samples: &[i16]) -> Vec<u8> {
    let mut bits = BitWriter::default();
    bits.write(1, 3); // a stereo pair element
    bits.write(0, 4);
    bits.write(0, 12);
    bits.write(0, 1); // no explicit size: the default 352 frames
    bits.write(0, 2);
    bits.write(1, 1); // not compressed
    for sample in samples {
        bits.write(*sample as u16 as u32, 16);
    }
    bits.write(7, 3); // end
    bits.finish()
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    current: u32,
    used: u32,
}
impl BitWriter {
    fn write(&mut self, value: u32, count: u32) {
        for i in (0..count).rev() {
            self.current = (self.current << 1) | ((value >> i) & 1);
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.current as u8);
                self.current = 0;
                self.used = 0;
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.used > 0 {
            self.bytes.push((self.current << (8 - self.used)) as u8);
        }
        self.bytes
    }
}

/// Seconds since 1900 as NTP's 32.32 fixed point.
fn ntp_now() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() + 2_208_988_800;
    let fraction = ((now.subsec_nanos() as u64) << 32) / 1_000_000_000;
    (seconds << 32) | fraction
}

// ---------------------------------------------------------------- RSA-OAEP for the AES key

fn mgf1(seed: &[u8], length: usize) -> Vec<u8> {
    let mut out = vec![];
    let mut counter = 0u32;
    while out.len() < length {
        let mut hash = sha1_smol::Sha1::new();
        hash.update(seed);
        hash.update(&counter.to_be_bytes());
        out.extend(hash.digest().bytes());
        counter += 1;
    }
    out.truncate(length);
    out
}

/// RSA-OAEP (SHA-1) encryption of `message` with Apple's key.
fn wrap_key(message: &[u8]) -> Result<Vec<u8>> {
    let modulus = B64.decode(APPLE_KEY)?;
    let k = modulus.len();
    let label_hash = sha1_smol::Sha1::from(b"").digest().bytes();
    let mut block = label_hash.to_vec();
    block.resize(k - message.len() - 2 - 20, 0);
    block.push(1);
    block.extend(message);
    let seed: [u8; 20] = rand::random();
    let masked_block: Vec<u8> = block
        .iter()
        .zip(mgf1(&seed, block.len()))
        .map(|(a, b)| a ^ b)
        .collect();
    let masked_seed: Vec<u8> = seed
        .iter()
        .zip(mgf1(&masked_block, 20))
        .map(|(a, b)| a ^ b)
        .collect();
    let mut encoded = vec![0u8];
    encoded.extend(masked_seed);
    encoded.extend(masked_block);
    let n = num_bigint::BigUint::from_bytes_be(&modulus);
    let c = num_bigint::BigUint::from_bytes_be(&encoded)
        .modpow(&num_bigint::BigUint::from(65_537u32), &n);
    let mut out = c.to_bytes_be();
    while out.len() < k {
        out.insert(0, 0);
    }
    Ok(out)
}

// ---------------------------------------------------------------- RTSP

struct Rtsp {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    url: String,
    sequence: u32,
    session: Option<String>,
    instance: String,
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
}
impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl Rtsp {
    fn request(
        &mut self,
        method: &str,
        headers: &[(&str, String)],
        body: Option<(&str, &[u8])>,
    ) -> Result<Reply> {
        self.sequence += 1;
        let mut text = format!(
            "{method} {} RTSP/1.0\r\nCSeq: {}\r\nUser-Agent: Needle/1.0\r\nClient-Instance: {}\r\nDACP-ID: {}\r\n",
            if method == "OPTIONS" { "*" } else { &self.url },
            self.sequence,
            self.instance,
            self.instance
        );
        if let Some(session) = &self.session {
            text.push_str(&format!("Session: {session}\r\n"));
        }
        for (k, v) in headers {
            text.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some((kind, bytes)) = body {
            text.push_str(&format!(
                "Content-Type: {kind}\r\nContent-Length: {}\r\n",
                bytes.len()
            ));
        }
        text.push_str("\r\n");
        self.stream.write_all(text.as_bytes())?;
        if let Some((_, bytes)) = body {
            self.stream.write_all(bytes)?;
        }
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        let status: u16 = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .context("The AirPlay receiver did not answer")?;
        let mut headers = vec![];
        loop {
            let mut header = String::new();
            if self.reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = header.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        let reply = Reply { status, headers };
        if let Some(length) = reply
            .header("Content-Length")
            .and_then(|l| l.parse::<usize>().ok())
        {
            let mut skip = vec![0; length];
            self.reader.read_exact(&mut skip)?;
        }
        Ok(reply)
    }
}

// ---------------------------------------------------------------- the receiver

type Cipher = cbc::Encryptor<aes::Aes128>;

/// Packets kept for resending when the receiver misses one.
struct Sent {
    packets: VecDeque<(u16, Vec<u8>)>,
}

pub struct Receiver {
    address: SocketAddr,
    local: IpAddr,
    rtsp: Option<Rtsp>,
    audio: Option<UdpSocket>,
    server: Option<SocketAddr>,
    control: Option<(UdpSocket, SocketAddr)>,
    key: Option<([u8; 16], [u8; 16])>,
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    first: bool,
    pending: Vec<i16>,
    sent: Arc<Mutex<Sent>>,
    stop: Arc<AtomicBool>,
    last_sync: std::time::Instant,
}

impl Receiver {
    pub fn new(speaker: &Speaker) -> Result<Self> {
        let address = speaker
            .socket()
            .context("The speaker's address is not valid")?;
        Ok(Self {
            address,
            local: super::local_ip_for(address)?,
            rtsp: None,
            audio: None,
            server: None,
            control: None,
            key: None,
            sequence: rand::random(),
            timestamp: rand::random(),
            ssrc: rand::random(),
            first: true,
            pending: vec![],
            sent: Arc::new(Mutex::new(Sent {
                packets: VecDeque::new(),
            })),
            stop: Arc::new(AtomicBool::new(false)),
            last_sync: std::time::Instant::now(),
        })
    }

    fn connect(&mut self, encrypt: bool) -> Result<()> {
        let stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(5))
            .context("The AirPlay receiver did not answer")?;
        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
        let session_id: u32 = rand::random();
        let mut rtsp = Rtsp {
            reader: BufReader::new(stream.try_clone()?),
            stream,
            url: format!("rtsp://{}/{session_id}", self.local),
            sequence: 0,
            session: None,
            instance: format!("{:016X}", rand::random::<u64>()),
        };
        let options = rtsp.request("OPTIONS", &[], None)?;
        if options.status == 401 {
            bail!("This AirPlay receiver asks for a password, which Needle does not support yet.");
        }
        let mut sdp = format!(
            "v=0\r\no=iTunes {session_id} 0 IN IP4 {}\r\ns=iTunes\r\nc=IN IP4 {}\r\nt=0 0\r\nm=audio 0 RTP/AVP 96\r\na=rtpmap:96 AppleLossless\r\na=fmtp:96 {FRAMES} 0 16 40 10 14 2 255 0 0 44100\r\n",
            self.local,
            self.address.ip()
        );
        self.key = None;
        if encrypt {
            let key: [u8; 16] = rand::random();
            let iv: [u8; 16] = rand::random();
            sdp.push_str(&format!(
                "a=rsaaeskey:{}\r\na=aesiv:{}\r\n",
                B64.encode(wrap_key(&key)?),
                B64.encode(iv)
            ));
            self.key = Some((key, iv));
        }
        let announce = rtsp.request("ANNOUNCE", &[], Some(("application/sdp", sdp.as_bytes())))?;
        if announce.status != 200 {
            bail!(
                "The AirPlay receiver refused the stream ({})",
                announce.status
            );
        }
        let control = UdpSocket::bind(("0.0.0.0", 0))?;
        let timing = UdpSocket::bind(("0.0.0.0", 0))?;
        let setup = rtsp.request(
            "SETUP",
            &[(
                "Transport",
                format!(
                    "RTP/AVP/UDP;unicast;interleaved=0-1;mode=record;control_port={};timing_port={}",
                    control.local_addr()?.port(),
                    timing.local_addr()?.port()
                ),
            )],
            None,
        )?;
        if setup.status != 200 {
            bail!("The AirPlay receiver refused to set up ({})", setup.status);
        }
        let transport = setup.header("Transport").unwrap_or_default().to_string();
        let port = |name: &str| -> Option<u16> {
            transport.split(';').find_map(|p| {
                p.strip_prefix(&format!("{name}="))
                    .and_then(|v| v.parse().ok())
            })
        };
        let server_port = port("server_port").context("The receiver gave no audio port")?;
        let control_port = port("control_port").unwrap_or(server_port + 1);
        rtsp.session = setup
            .header("Session")
            .map(|s| s.split(';').next().unwrap_or(s).to_string());
        let record = rtsp.request(
            "RECORD",
            &[
                ("Range", "npt=0-".into()),
                (
                    "RTP-Info",
                    format!("seq={};rtptime={}", self.sequence, self.timestamp),
                ),
            ],
            None,
        )?;
        if record.status != 200 {
            bail!("The AirPlay receiver refused to play ({})", record.status);
        }
        // Needle's own volume applies before the audio is sent; the receiver stays at full.
        let _ = rtsp.request(
            "SET_PARAMETER",
            &[],
            Some(("text/parameters", b"volume: 0.000000\r\n")),
        );

        let audio = UdpSocket::bind(("0.0.0.0", 0))?;
        self.server = Some(SocketAddr::new(self.address.ip(), server_port));
        self.audio = Some(audio);
        let control_peer = SocketAddr::new(self.address.ip(), control_port);
        self.control = Some((control.try_clone()?, control_peer));
        self.serve_timing(timing);
        self.serve_resends(control);
        self.rtsp = Some(rtsp);
        self.first = true;
        Ok(())
    }

    /// Answer the receiver's clock questions.
    fn serve_timing(&self, socket: UdpSocket) {
        let stop = self.stop.clone();
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        std::thread::spawn(move || {
            let mut buffer = [0u8; 128];
            while !stop.load(Ordering::Relaxed) {
                let Ok((n, peer)) = socket.recv_from(&mut buffer) else {
                    continue;
                };
                if n < 32 || buffer[1] & 0x7f != 0x52 {
                    continue;
                }
                let received = ntp_now();
                let mut reply = vec![0x80, 0xd3, 0x00, 0x07, 0, 0, 0, 0];
                reply.extend(&buffer[24..32]); // their send time, as our reference
                reply.extend(received.to_be_bytes());
                reply.extend(ntp_now().to_be_bytes());
                let _ = socket.send_to(&reply, peer);
            }
        });
    }

    /// Resend packets the receiver asks for again.
    fn serve_resends(&self, socket: UdpSocket) {
        let (stop, sent) = (self.stop.clone(), self.sent.clone());
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        std::thread::spawn(move || {
            let mut buffer = [0u8; 128];
            while !stop.load(Ordering::Relaxed) {
                let Ok((n, peer)) = socket.recv_from(&mut buffer) else {
                    continue;
                };
                if n < 8 || buffer[1] & 0x7f != 0x55 {
                    continue;
                }
                let first = u16::from_be_bytes([buffer[4], buffer[5]]);
                let count = u16::from_be_bytes([buffer[6], buffer[7]]);
                let sent = sent.lock().unwrap();
                for i in 0..count {
                    let wanted = first.wrapping_add(i);
                    if let Some((_, packet)) = sent.packets.iter().find(|(s, _)| *s == wanted) {
                        let mut resend = vec![0x80, 0xd6, 0x00, 0x01];
                        resend.extend(packet);
                        let _ = socket.send_to(&resend, peer);
                    }
                }
            }
        });
    }

    fn sync(&mut self) {
        let Some((socket, peer)) = &self.control else {
            return;
        };
        let mut packet = vec![if self.first { 0x90 } else { 0x80 }, 0xd4, 0x00, 0x07];
        packet.extend(self.timestamp.wrapping_sub(LATENCY).to_be_bytes());
        packet.extend(ntp_now().to_be_bytes());
        packet.extend(self.timestamp.to_be_bytes());
        let _ = socket.send_to(&packet, peer);
        self.last_sync = std::time::Instant::now();
    }

    fn send_packet(&mut self, frames: &[i16]) -> Result<()> {
        let mut payload = alac(frames);
        if let Some((key, iv)) = &self.key {
            // Whole 16-byte blocks are encrypted; a shorter tail stays as it is.
            let whole = payload.len() / 16 * 16;
            let mut cipher = Cipher::new(key.into(), iv.into());
            for block in payload[..whole].as_chunks_mut::<16>().0 {
                cipher.encrypt_block_mut(GenericArray::from_mut_slice(block));
            }
        }
        let mut packet = vec![0x80, if self.first { 0xe0 } else { 0x60 }];
        packet.extend(self.sequence.to_be_bytes());
        packet.extend(self.timestamp.to_be_bytes());
        packet.extend(self.ssrc.to_be_bytes());
        packet.extend(payload);
        if self.first || self.last_sync.elapsed() > Duration::from_secs(1) {
            self.sync();
        }
        if let (Some(socket), Some(server)) = (&self.audio, self.server) {
            socket.send_to(&packet, server)?;
        }
        let mut sent = self.sent.lock().unwrap();
        sent.packets.push_back((self.sequence, packet));
        if sent.packets.len() > 1000 {
            sent.packets.pop_front();
        }
        drop(sent);
        self.first = false;
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(FRAMES as u32);
        Ok(())
    }

    fn flush_receiver(&mut self) -> Result<()> {
        self.pending.clear();
        let info = format!("seq={};rtptime={}", self.sequence, self.timestamp);
        if let Some(rtsp) = self.rtsp.as_mut() {
            rtsp.request("FLUSH", &[("RTP-Info", info)], None)?;
        }
        self.first = true;
        Ok(())
    }
}

impl Destination for Receiver {
    fn start(&mut self, _: &Meta) -> Result<()> {
        // Unencrypted first; receivers that insist on encryption refuse, and get it.
        match self.connect(false) {
            Ok(()) => Ok(()),
            Err(first) => self.connect(true).map_err(|_| first),
        }
    }
    fn push(&mut self, samples: &[i16]) -> Result<()> {
        self.pending.extend(samples);
        let size = FRAMES * 2;
        while self.pending.len() >= size {
            let frames: Vec<i16> = self.pending.drain(..size).collect();
            self.send_packet(&frames)?;
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.flush_receiver()
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
    fn flush(&mut self, _: &Meta) -> Result<()> {
        self.flush_receiver()
    }
    fn tick(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(rtsp) = self.rtsp.as_mut() {
            let _ = rtsp.request("TEARDOWN", &[], None);
        }
    }
    fn lag(&self) -> f64 {
        LATENCY as f64 / 44_100.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alac_frames_have_the_uncompressed_layout() {
        let samples: Vec<i16> = (0..FRAMES * 2).map(|i| i as i16 - 300).collect();
        let frame = alac(&samples);
        // 23 header bits + 352 × 2 × 16 sample bits + 3 end bits, rounded up to bytes.
        assert_eq!(frame.len(), (23 + FRAMES * 32 + 3).div_ceil(8));
        // Tag 001, then zeros, then the "not compressed" bit at position 22.
        assert_eq!(frame[0], 0b0010_0000);
        assert_eq!(frame[1], 0);
        assert_eq!(frame[2] & 0b10, 0b10);
        // The first sample (-300 = 0xFED4) starts at bit 23.
        let first =
            ((frame[2] as u32 & 1) << 15) | ((frame[3] as u32) << 7) | (frame[4] as u32 >> 1);
        assert_eq!(first as u16, (-300i16) as u16);
    }

    #[test]
    fn apple_key_is_2048_bits_and_wraps() {
        assert_eq!(B64.decode(APPLE_KEY).unwrap().len(), 256);
        let wrapped = wrap_key(&[7u8; 16]).unwrap();
        assert_eq!(wrapped.len(), 256);
        assert_ne!(wrap_key(&[7u8; 16]).unwrap(), wrapped, "OAEP is randomised");
    }

    #[test]
    fn ntp_time_is_after_1900() {
        assert!(ntp_now() >> 32 > 3_900_000_000);
    }

    /// A pretend receiver: answers RTSP, and counts audio packets.
    #[test]
    fn sets_up_a_session_and_sends_audio() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let rtsp_port = listener.local_addr().unwrap().port();
        let audio = UdpSocket::bind("127.0.0.1:0").unwrap();
        audio
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let audio_port = audio.local_addr().unwrap().port();
        let methods: Arc<Mutex<Vec<String>>> = Arc::default();
        let seen = methods.clone();
        std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut socket = socket;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let method = line.split_whitespace().next().unwrap_or("").to_string();
                let (mut cseq, mut length) = (String::new(), 0);
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = header.strip_prefix("CSeq:") {
                        cseq = v.trim().to_string();
                    }
                    if let Some(v) = header.strip_prefix("Content-Length:") {
                        length = v.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                seen.lock().unwrap().push(method.clone());
                let extra = if method == "SETUP" {
                    format!(
                        "Transport: RTP/AVP/UDP;unicast;mode=record;server_port={audio_port};control_port={audio_port};timing_port={audio_port}\r\nSession: 1\r\n"
                    )
                } else {
                    String::new()
                };
                write!(socket, "RTSP/1.0 200 OK\r\nCSeq: {cseq}\r\n{extra}\r\n").unwrap();
            }
        });
        let speaker = Speaker {
            kind: super::super::Kind::AirPlay,
            name: "Test".into(),
            address: format!("127.0.0.1:{rtsp_port}"),
        };
        let mut receiver = Receiver::new(&speaker).unwrap();
        receiver.start(&Meta::default()).unwrap();
        receiver.push(&vec![0i16; FRAMES * 2 * 3 + 10]).unwrap();
        let mut packets = 0;
        let mut buffer = [0u8; 2048];
        while let Ok((n, _)) = audio.recv_from(&mut buffer) {
            // Audio packets carry payload type 96 (0x60, with the marker bit on the first).
            if n > 100 && buffer[1] & 0x7f == 0x60 {
                packets += 1;
                if packets == 3 {
                    break;
                }
            }
        }
        assert_eq!(packets, 3);
        receiver.stop();
        std::thread::sleep(Duration::from_millis(200));
        let methods = methods.lock().unwrap().clone();
        assert_eq!(
            methods,
            vec![
                "OPTIONS",
                "ANNOUNCE",
                "SETUP",
                "RECORD",
                "SET_PARAMETER",
                "TEARDOWN"
            ]
        );
    }
}
