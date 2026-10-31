//! "Copy DAC details": what Needle read from a USB DAC, in words and in hex, for a report. The
//! raw descriptors hold no strings (a name or a serial number is fetched apart), so nothing
//! personal is in it.
use super::descriptors::{Dac, Format, Path, Sync, Uac, listed_rates, parse, rank};
use std::fmt::Write;

/// A terminal type in words.
fn terminal(kind: u16) -> &'static str {
    match kind {
        0x0101 => "USB streaming",
        0x0301 => "speaker",
        0x0302 => "headphones",
        0x0303 => "head-mounted display",
        0x0304 => "desktop speaker",
        0x0305 => "room speaker",
        0x0306 => "communication speaker",
        0x0307 => "low-frequency speaker",
        0x0401 => "handset",
        0x0402 => "headset",
        0x0403 => "speakerphone",
        0x0404 => "echo-suppressing speakerphone",
        0x0405 => "echo-cancelling speakerphone",
        0x0603 => "line connector",
        0x0605 => "S/PDIF",
        k => match k >> 8 {
            0x03 => "output",
            0x04 => "bidirectional",
            0x05 => "telephony",
            0x06 => "external",
            0x07 => "embedded",
            _ => "unknown",
        },
    }
}

fn speed(speed: i32) -> String {
    match speed {
        1 => "low".into(),
        2 => "full (12 Mb/s)".into(),
        3 => "high (480 Mb/s)".into(),
        4 => "wireless".into(),
        5 => "super (5 Gb/s)".into(),
        6 => "super+ (10 Gb/s)".into(),
        n => format!("unknown ({n})"),
    }
}

fn rates(f: &Format) -> String {
    match f.range {
        Some((low, high)) => {
            format!(
                "a range, {low} to {high} Hz (Needle trusts {:?})",
                listed_rates(f)
            )
        }
        None if f.rates.is_empty() => "from the clock".into(),
        None if listed_rates(f) != f.rates => format!(
            "a list, {:?} Hz, with no rate control (Needle trusts {:?})",
            f.rates,
            listed_rates(f)
        ),
        None => format!("a list, {:?} Hz", f.rates),
    }
}

fn format_line(f: &Format) -> String {
    format!(
        "setting {}: {} channels, {} bits in {}-byte slots, rates: {}, {} sync, feedback {}, rate control {}, endpoint {:#04x}, packets up to {} bytes, interval {}",
        f.alt,
        f.channels,
        f.bits,
        f.subslot,
        rates(f),
        match f.sync {
            Sync::Async => "async",
            Sync::Adaptive => "adaptive",
            Sync::Synchronous => "synchronous",
            Sync::None => "no",
        },
        f.feedback
            .map_or("none".to_string(), |e| format!("{e:#04x}")),
        if f.rate_control { "yes" } else { "no" },
        f.endpoint,
        f.max_packet,
        f.interval,
    )
}

fn path_line(p: &Path, chosen: bool) -> String {
    format!(
        "{} interface {}, from terminal {}{} to {}, volume {}",
        if chosen { "*" } else { "-" },
        p.interface,
        p.terminal,
        p.units
            .iter()
            .fold(String::new(), |text, unit| format!(" unit {unit},{text}")),
        p.output
            .map_or("no output terminal".to_string(), |(id, kind)| {
                format!(
                    "output {id}: {} ({kind:#06x}), rank {}",
                    terminal(kind),
                    rank(Some(kind))
                )
            }),
        p.volume.map_or("none".to_string(), |v| format!(
            "unit {} ({})",
            v.unit,
            if v.master { "master" } else { "each channel" }
        )),
    )
}

/// Every way through the DAC and its formats, one line each; `*` marks the one used.
pub fn paths(dac: &Dac) -> Vec<String> {
    let mut lines = vec![];
    for (i, path) in dac.paths.iter().enumerate() {
        lines.push(path_line(path, i == dac.chosen));
        lines.extend(
            path.formats
                .iter()
                .map(|f| format!("    {}", format_line(f))),
        );
    }
    lines
}

/// Everything known of a DAC. `speed` is Linux's number for it; `driver` is what the driver in
/// use adds.
pub fn text(
    raw: &[u8],
    name: &str,
    vendor: u16,
    product: u16,
    speed_number: Option<i32>,
    driver: Option<String>,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "DAC: {name} ({vendor:04x}:{product:04x})");
    let _ = writeln!(
        out,
        "USB speed: {}",
        speed_number.map_or("not known".to_string(), speed)
    );
    match parse(raw) {
        Some(dac) => {
            let _ = writeln!(
                out,
                "USB Audio Class {}, audio control interface {}",
                if dac.uac == Uac::Two { 2 } else { 1 },
                dac.control
            );
            let _ = writeln!(
                out,
                "Volume: {}",
                dac.volume
                    .map_or("none on the chosen path".to_string(), |v| format!(
                        "unit {}",
                        v.unit
                    ))
            );
            let _ = writeln!(out, "Paths (* is the one Needle uses):");
            for line in paths(&dac) {
                let _ = writeln!(out, "{line}");
            }
        }
        None => {
            let _ = writeln!(out, "Needle could not read this as a USB audio device.");
        }
    }
    if let Some(driver) = driver {
        out += &driver;
    }
    let _ = writeln!(out, "Raw descriptors ({} bytes):", raw.len());
    for row in raw.chunks(32) {
        let line: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
        let _ = writeln!(out, "{}", line.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::fake::{Dock, dock, uac1_descriptors};
    use super::*;

    #[test]
    fn the_details_of_a_dock_say_every_path_and_which_is_used() {
        let raw = dock(Dock::default());
        let text = text(
            &raw,
            "Pretend dock",
            0x0bda,
            0x4852,
            Some(2),
            Some("Chosen: x\n".into()),
        );
        let has = |words: &str| assert!(text.contains(words), "{words} is not in:\n{text}");
        has("DAC: Pretend dock (0bda:4852)");
        has("USB speed: full (12 Mb/s)");
        has("USB Audio Class 1, audio control interface 0");
        has(
            "- interface 1, from terminal 11 unit 12, to output 13: headset (0x0402), rank 1, volume unit 12 (master)",
        );
        has(
            "* interface 3, from terminal 21 unit 22, to output 23: speaker (0x0301), rank 3, volume unit 22 (master)",
        );
        has(
            "setting 1: 2 channels, 16 bits in 2-byte slots, rates: a range, 8000 to 48000 Hz (Needle trusts [48000])",
        );
        has("rates: a list, [44100, 48000] Hz, adaptive sync, feedback none, rate control yes");
        has("Chosen: x");
        // The hex, every byte of it.
        let hex: Vec<u8> = text
            .split("bytes):\n")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect();
        assert_eq!(hex, raw);
    }

    #[test]
    fn the_details_say_when_a_listed_rate_is_not_trusted() {
        let raw = uac1_descriptors(&[(1, &[44_100, 48_000], false)]);
        let text = text(&raw, "Pretend DAC", 1, 2, None, None);
        let words = "a list, [44100, 48000] Hz, with no rate control (Needle trusts [48000])";
        assert!(text.contains(words), "{text}");
    }

    #[test]
    fn descriptors_that_make_no_sense_still_give_their_hex() {
        let text = text(&[1, 2, 3], "Odd", 1, 2, None, None);
        assert!(text.contains("could not read"));
        assert!(text.contains("01 02 03"));
        assert!(text.contains("USB speed: not known"));
    }
}
