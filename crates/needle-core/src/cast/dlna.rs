//! DLNA / UPnP renderers: TVs, receivers, and speakers that play a URL they are handed.
use super::{Meta, Remote, xml_escape};
use anyhow::{Context, Result, bail};
use std::time::Duration;

const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";

/// The text inside the first `<tag>…</tag>` (any namespace prefix).
pub(crate) fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = xml;
    loop {
        let open = rest.find('<')?;
        rest = &rest[open + 1..];
        let end = rest.find('>')?;
        let head = &rest[..end];
        let local = head.split_whitespace().next()?.rsplit(':').next()?;
        if local == name && !head.starts_with('/') && !head.ends_with('/') {
            let body = &rest[end + 1..];
            let close = body.find("</")?;
            return Some(body[..close].trim());
        }
        rest = &rest[end + 1..];
    }
}

/// Resolve `url` against the description's address.
fn absolute(base: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    let root = base
        .find("://")
        .and_then(|i| base[i + 3..].find('/').map(|j| &base[..i + 3 + j]))
        .unwrap_or(base);
    if url.starts_with('/') {
        format!("{root}{url}")
    } else {
        format!("{root}/{url}")
    }
}

pub struct Renderer {
    control: String,
    client: reqwest::blocking::Client,
}

/// The friendly name and AV transport control URL from a device description.
pub(crate) fn describe(description: &str, xml: &str) -> Option<(String, String)> {
    let name = tag(xml, "friendlyName").unwrap_or("Speaker").to_string();
    let base = tag(xml, "URLBase")
        .map(str::to_string)
        .unwrap_or_else(|| description.to_string());
    let control = xml
        .split("<service>")
        .skip(1)
        .chain(xml.split("<service ").skip(1))
        .find_map(|service| {
            let kind = tag(service, "serviceType")?;
            kind.starts_with("urn:schemas-upnp-org:service:AVTransport:")
                .then(|| tag(service, "controlURL"))
                .flatten()
        })?;
    Some((name, absolute(&base, control)))
}

impl Renderer {
    pub fn connect(description: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()?;
        let xml = client.get(description).send()?.error_for_status()?.text()?;
        let (_, control) = describe(description, &xml)
            .context("This speaker does not accept streams (no AVTransport)")?;
        Ok(Self { control, client })
    }

    fn call(&self, action: &str, arguments: &[(&str, &str)]) -> Result<String> {
        let body: String = arguments
            .iter()
            .map(|(k, v)| format!("<{k}>{}</{k}>", xml_escape(v)))
            .collect();
        let envelope = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{AV_TRANSPORT}\">{body}</u:{action}></s:Body></s:Envelope>"
        );
        let reply = self
            .client
            .post(&self.control)
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", format!("\"{AV_TRANSPORT}#{action}\""))
            .body(envelope)
            .send()?;
        let status = reply.status();
        let text = reply.text().unwrap_or_default();
        if !status.is_success() {
            let why = tag(&text, "errorDescription").unwrap_or("no reason given");
            bail!("The speaker refused {action}: {why}");
        }
        Ok(text)
    }
}

/// DIDL-Lite metadata describing the stream.
pub(crate) fn didl(url: &str, meta: &Meta) -> String {
    let cover = meta
        .cover
        .as_ref()
        .map(|c| format!("<upnp:albumArtURI>{}</upnp:albumArtURI>", xml_escape(c)))
        .unwrap_or_default();
    format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\"><item id=\"1\" parentID=\"0\" restricted=\"1\"><dc:title>{}</dc:title><upnp:artist>{}</upnp:artist><dc:creator>{}</dc:creator><upnp:album>{}</upnp:album>{cover}<upnp:class>object.item.audioItem.musicTrack</upnp:class><res protocolInfo=\"http-get:*:audio/wav:DLNA.ORG_PN=WAV;DLNA.ORG_OP=00;DLNA.ORG_FLAGS=01700000000000000000000000000000\">{}</res></item></DIDL-Lite>",
        xml_escape(&meta.title),
        xml_escape(&meta.artist),
        xml_escape(&meta.artist),
        xml_escape(&meta.album),
        xml_escape(url)
    )
}

impl Remote for Renderer {
    fn load(&mut self, url: &str, meta: &Meta) -> Result<()> {
        // Some renderers refuse a new URL while playing.
        let _ = self.call("Stop", &[("InstanceID", "0")]);
        self.call(
            "SetAVTransportURI",
            &[
                ("InstanceID", "0"),
                ("CurrentURI", url),
                ("CurrentURIMetaData", &didl(url, meta)),
            ],
        )?;
        self.call("Play", &[("InstanceID", "0"), ("Speed", "1")])?;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.call("Pause", &[("InstanceID", "0")]).map(drop)
    }
    fn resume(&mut self) -> Result<()> {
        self.call("Play", &[("InstanceID", "0"), ("Speed", "1")])
            .map(drop)
    }
    fn stop(&mut self) -> Result<()> {
        self.call("Stop", &[("InstanceID", "0")]).map(drop)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
    };

    pub const DESCRIPTION: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0"><device>
<deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
<friendlyName>Kitchen &amp; Co</friendlyName>
<serviceList>
<service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/rc</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/upnp/control/AVTransport1</controlURL></service>
</serviceList></device></root>"#;

    /// A pretend renderer: serves its description and records the actions it is sent.
    type Log = Arc<Mutex<Vec<(String, String)>>>;

    pub fn fake_renderer() -> (String, Log) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: Log = Arc::default();
        let seen = log.clone();
        std::thread::spawn(move || {
            for mut connection in listener.incoming().flatten() {
                let mut reader = BufReader::new(connection.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (mut length, mut action) = (0, String::new());
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim().is_empty() {
                        break;
                    }
                    let lower = header.to_lowercase();
                    if let Some(v) = lower.strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if lower.starts_with("soapaction:") {
                        action = header
                            .split('#')
                            .nth(1)
                            .unwrap_or("")
                            .trim()
                            .trim_end_matches('"')
                            .to_string();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let reply = if line.starts_with("GET /desc.xml") {
                    DESCRIPTION.to_string()
                } else {
                    seen.lock()
                        .unwrap()
                        .push((action.clone(), String::from_utf8_lossy(&body).to_string()));
                    format!("<s:Envelope><s:Body><u:{action}Response/></s:Body></s:Envelope>")
                };
                write!(
                    connection,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
        });
        (format!("http://127.0.0.1:{port}/desc.xml"), log)
    }

    #[test]
    fn reads_descriptions_and_drives_a_renderer() {
        let (name, control) = describe("http://10.0.0.5:1400/xml/device.xml", DESCRIPTION).unwrap();
        assert_eq!(name, "Kitchen &amp; Co");
        assert_eq!(control, "http://10.0.0.5:1400/upnp/control/AVTransport1");
        assert_eq!(absolute("http://h:1/a/b.xml", "c"), "http://h:1/c");

        let (description, log) = fake_renderer();
        let mut renderer = Renderer::connect(&description).unwrap();
        let meta = Meta {
            title: "Rock & Roll".into(),
            artist: "A".into(),
            album: "B".into(),
            cover: None,
        };
        renderer
            .load("http://10.0.0.2:5000/live/1.wav", &meta)
            .unwrap();
        renderer.pause().unwrap();
        renderer.resume().unwrap();
        let log = log.lock().unwrap();
        let actions: Vec<&str> = log.iter().map(|(a, _)| a.as_str()).collect();
        assert_eq!(
            actions,
            vec!["Stop", "SetAVTransportURI", "Play", "Pause", "Play"]
        );
        let set = &log[1].1;
        assert!(set.contains("<CurrentURI>http://10.0.0.2:5000/live/1.wav</CurrentURI>"));
        // The metadata is escaped twice: once as DIDL, once inside SOAP.
        assert!(set.contains("Rock &amp;amp; Roll"));
    }
}
