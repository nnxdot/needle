//! Finding speakers: SSDP for DLNA renderers, and multicast DNS for Chromecast and AirPlay.
use super::{Kind, Speaker};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

/// Every speaker that answers within `wait`.
pub fn discover(wait: Duration) -> Vec<Speaker> {
    let ssdp = std::thread::spawn(move || dlna(wait));
    let mdns = std::thread::spawn(move || mdns(wait));
    let mut found = ssdp.join().unwrap_or_default();
    found.extend(mdns.join().unwrap_or_default());
    found.sort_by_key(|s| s.name.to_lowercase());
    found.dedup_by(|a, b| a.kind == b.kind && a.address == b.address);
    found
}

// ---------------------------------------------------------------- SSDP

fn dlna(wait: Duration) -> Vec<Speaker> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return vec![];
    };
    let _ = socket.set_multicast_ttl_v4(2);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let search = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
    for _ in 0..2 {
        let _ = socket.send_to(search.as_bytes(), "239.255.255.250:1900");
    }
    let mut locations = vec![];
    let end = Instant::now() + wait;
    let mut buffer = [0u8; 4096];
    while Instant::now() < end {
        let Ok((n, _)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let text = String::from_utf8_lossy(&buffer[..n]);
        if let Some(location) = text.lines().find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("location"))
                .map(|(_, v)| v.trim().to_string())
        }) && !locations.contains(&location)
        {
            locations.push(location);
        }
    }
    let lookups: Vec<_> = locations
        .into_iter()
        .map(|location| {
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(3))
                    .build()
                    .ok()?;
                let xml = client.get(&location).send().ok()?.text().ok()?;
                let (name, _) = super::dlna::describe(&location, &xml)?;
                Some(Speaker {
                    kind: Kind::Dlna,
                    name: unescape(&name),
                    address: location,
                })
            })
        })
        .collect();
    lookups
        .into_iter()
        .filter_map(|l| l.join().ok().flatten())
        .collect()
}

pub(crate) fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------- multicast DNS

const CAST: &str = "_googlecast._tcp.local";
const RAOP: &str = "_raop._tcp.local";

fn query(name: &str) -> Vec<u8> {
    let mut packet = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        packet.push(label.len() as u8);
        packet.extend(label.as_bytes());
    }
    packet.push(0);
    packet.extend([0, 12, 0, 1]); // PTR, IN
    packet
}

/// A name at `offset`, following compression pointers. Returns the name and the offset after
/// it (in the original position).
fn read_name(packet: &[u8], mut offset: usize) -> Option<(String, usize)> {
    let mut labels = vec![];
    let mut after = None;
    for _ in 0..64 {
        let length = *packet.get(offset)? as usize;
        if length == 0 {
            return Some((labels.join("."), after.unwrap_or(offset + 1)));
        }
        if length & 0xc0 == 0xc0 {
            let pointer = ((length & 0x3f) << 8) | *packet.get(offset + 1)? as usize;
            after.get_or_insert(offset + 2);
            offset = pointer;
            continue;
        }
        labels.push(
            String::from_utf8_lossy(packet.get(offset + 1..offset + 1 + length)?).to_string(),
        );
        offset += 1 + length;
    }
    None
}

#[derive(Debug, Default)]
pub(crate) struct Records {
    pub pointers: Vec<(String, String)>,
    pub services: HashMap<String, (String, u16)>,
    pub texts: HashMap<String, Vec<String>>,
    pub addresses: HashMap<String, IpAddr>,
}

pub(crate) fn parse(packet: &[u8], records: &mut Records) -> Option<()> {
    let count = |i: usize| -> Option<usize> {
        Some(u16::from_be_bytes([*packet.get(i)?, *packet.get(i + 1)?]) as usize)
    };
    let (questions, answers) = (count(4)?, count(6)? + count(8)? + count(10)?);
    let mut offset = 12;
    for _ in 0..questions {
        offset = read_name(packet, offset)?.1 + 4;
    }
    for _ in 0..answers {
        let (name, after) = read_name(packet, offset)?;
        let kind = count(after)?;
        let length = count(after + 8)?;
        let data = after + 10;
        let body = packet.get(data..data + length)?;
        match kind {
            12 => records
                .pointers
                .push((name.to_lowercase(), read_name(packet, data)?.0)),
            33 => {
                let port = u16::from_be_bytes([body[4], body[5]]);
                records.services.insert(
                    name.to_lowercase(),
                    (read_name(packet, data + 6)?.0.to_lowercase(), port),
                );
            }
            16 => {
                let mut texts = vec![];
                let mut i = 0;
                while i < body.len() {
                    let n = body[i] as usize;
                    texts.push(String::from_utf8_lossy(body.get(i + 1..i + 1 + n)?).to_string());
                    i += 1 + n;
                }
                records.texts.insert(name.to_lowercase(), texts);
            }
            1 if length == 4 => {
                records.addresses.insert(
                    name.to_lowercase(),
                    IpAddr::from([body[0], body[1], body[2], body[3]]),
                );
            }
            _ => {}
        }
        offset = data + length;
    }
    Some(())
}

/// Speakers from the collected records; `from` is where each instance's answer came from, for
/// devices that leave out their address.
pub(crate) fn speakers(records: &Records, from: &HashMap<String, IpAddr>) -> Vec<Speaker> {
    let mut found = vec![];
    for (service, instance) in &records.pointers {
        let kind = match service.as_str() {
            s if s == CAST => Kind::Chromecast,
            s if s == RAOP => Kind::AirPlay,
            _ => continue,
        };
        let key = instance.to_lowercase();
        let Some((host, port)) = records.services.get(&key) else {
            continue;
        };
        let Some(ip) = records.addresses.get(host).or_else(|| from.get(&key)) else {
            continue;
        };
        let text = records.texts.get(&key).cloned().unwrap_or_default();
        let label = instance.split('.').next().unwrap_or(instance);
        let name = match kind {
            Kind::Chromecast => text
                .iter()
                .find_map(|t| t.strip_prefix("fn="))
                .map(str::to_string)
                .unwrap_or_else(|| label.to_string()),
            // "0011AABBCCDD@Living Room"
            _ => label.split_once('@').map_or(label, |(_, n)| n).to_string(),
        };
        found.push(Speaker {
            kind,
            name,
            address: SocketAddr::new(*ip, *port).to_string(),
        });
    }
    found
}

fn mdns(wait: Duration) -> Vec<Speaker> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return vec![];
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    for name in [CAST, RAOP] {
        let _ = socket.send_to(&query(name), "224.0.0.251:5353");
    }
    let mut records = Records::default();
    let mut from = HashMap::new();
    let end = Instant::now() + wait;
    let mut buffer = [0u8; 9000];
    while Instant::now() < end {
        let Ok((n, peer)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let before = records.pointers.len();
        parse(&buffer[..n], &mut records);
        for (_, instance) in &records.pointers[before..] {
            from.insert(instance.to_lowercase(), peer.ip());
        }
    }
    speakers(&records, &from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(out: &mut Vec<u8>, name: &str) {
        for label in name.split('.') {
            out.push(label.len() as u8);
            out.extend(label.as_bytes());
        }
        out.push(0);
    }
    fn record(out: &mut Vec<u8>, owner: &str, kind: u16, data: &[u8]) {
        name(out, owner);
        out.extend(kind.to_be_bytes());
        out.extend([0x80, 1, 0, 0, 0, 120]);
        out.extend((data.len() as u16).to_be_bytes());
        out.extend(data);
    }

    #[test]
    fn reads_a_chromecast_and_an_airplay_answer() {
        let mut packet = vec![0, 0, 0x84, 0, 0, 0, 0, 5, 0, 0, 0, 0];
        let mut target = vec![];
        name(&mut target, "Chromecast-abc._googlecast._tcp.local");
        record(&mut packet, CAST, 12, &target);
        let mut srv = vec![0, 0, 0, 0, 0x1f, 0x49];
        name(&mut srv, "abc.local");
        record(
            &mut packet,
            "Chromecast-abc._googlecast._tcp.local",
            33,
            &srv,
        );
        let mut txt = vec![];
        for t in ["id=1", "fn=Living Room TV"] {
            txt.push(t.len() as u8);
            txt.extend(t.as_bytes());
        }
        record(
            &mut packet,
            "Chromecast-abc._googlecast._tcp.local",
            16,
            &txt,
        );
        record(&mut packet, "abc.local", 1, &[192, 168, 1, 40]);
        let mut raop = vec![];
        name(&mut raop, "0011AABBCCDD@Kitchen._raop._tcp.local");
        record(&mut packet, RAOP, 12, &raop);
        let mut records = Records::default();
        parse(&packet, &mut records).unwrap();
        let mut srv = vec![0, 0, 0, 0, 0x1b, 0x58];
        let mut second = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        name(&mut srv, "kitchen.local");
        record(
            &mut second,
            "0011AABBCCDD@Kitchen._raop._tcp.local",
            33,
            &srv,
        );
        parse(&second, &mut records).unwrap();
        let from = HashMap::from([(
            "0011aabbccdd@kitchen._raop._tcp.local".to_string(),
            IpAddr::from([192, 168, 1, 50]),
        )]);
        let mut found = speakers(&records, &from);
        found.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            Speaker {
                kind: Kind::AirPlay,
                name: "Kitchen".into(),
                address: "192.168.1.50:7000".into()
            }
        );
        assert_eq!(
            found[1],
            Speaker {
                kind: Kind::Chromecast,
                name: "Living Room TV".into(),
                address: "192.168.1.40:8009".into()
            }
        );
        assert_eq!(unescape("A &amp; B"), "A & B");
    }

    #[test]
    fn follows_compressed_names() {
        // "local" at 12, then "x" + pointer to 12.
        let mut packet = vec![0u8; 12];
        packet.extend([5, b'l', b'o', b'c', b'a', b'l', 0, 1, b'x', 0xc0, 12]);
        assert_eq!(read_name(&packet, 19), Some(("x.local".to_string(), 23)));
    }
}
