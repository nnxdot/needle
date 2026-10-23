//! Needle's log file and crash reports.
//!
//! The log is `<data>/logs/needle.log`; when it passes 1 MB it moves to `needle.1.log` (and
//! that to `needle.2.log`), so at most about 3 MB is kept. When Needle crashes, the panic
//! hook writes a report to `<data>/crashes/`. On the next start, reports are sent to
//! needle.nnx.fyi unless crash reports are turned off, with file paths and the user's name
//! taken out; sent reports are deleted, and unsent ones are deleted after 30 days.
use anyhow::Result;
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime},
};

pub const CRASH_URL: &str = "https://needle.nnx.fyi/api/crash";
const LOG_LIMIT: u64 = 1 << 20;
const KEEP_LOGS: usize = 2;
const MAX_REPORT: usize = 24 * 1024;
const KEEP_UNSENT: Duration = Duration::from_secs(30 * 24 * 3600);

struct Log {
    folder: PathBuf,
    file: Option<File>,
    written: u64,
}

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();
static CRASHES: OnceLock<PathBuf> = OnceLock::new();

pub fn log_folder(data: &Path) -> PathBuf {
    data.join("logs")
}
pub fn crash_folder(data: &Path) -> PathBuf {
    data.join("crashes")
}

/// Start the log and the crash reporter for the library in `data`.
pub fn init(data: &Path) {
    let folder = log_folder(data);
    let _ = fs::create_dir_all(&folder);
    let _ = fs::create_dir_all(crash_folder(data));
    let _ = CRASHES.set(crash_folder(data));
    if LOG
        .set(Mutex::new(Log {
            folder,
            file: None,
            written: 0,
        }))
        .is_ok()
    {
        info(format!(
            "Needle {} started on {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    install_panic_hook();
}

fn write(level: &str, message: &str) {
    let Some(log) = LOG.get() else { return };
    let Ok(mut log) = log.lock() else { return };
    if log.file.is_none() || log.written > LOG_LIMIT {
        let path = log.folder.join("needle.log");
        if log.written > LOG_LIMIT || fs::metadata(&path).is_ok_and(|m| m.len() > LOG_LIMIT) {
            log.file = None;
            for n in (1..KEEP_LOGS).rev() {
                let _ = fs::rename(
                    log.folder.join(format!("needle.{n}.log")),
                    log.folder.join(format!("needle.{}.log", n + 1)),
                );
            }
            let _ = fs::rename(&path, log.folder.join("needle.1.log"));
        }
        log.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        log.written = fs::metadata(&path).map_or(0, |m| m.len());
    }
    let line = format!(
        "{} {level} {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        message.replace('\n', "\n    ")
    );
    if let Some(file) = log.file.as_mut()
        && file.write_all(line.as_bytes()).is_ok()
    {
        log.written += line.len() as u64;
    }
}

pub fn info(message: impl AsRef<str>) {
    write("INFO ", message.as_ref());
}
pub fn warn(message: impl AsRef<str>) {
    write("WARN ", message.as_ref());
}
pub fn error(message: impl AsRef<str>) {
    write("ERROR", message.as_ref());
}

fn install_panic_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if INSTALLED.set(()).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let backtrace = std::backtrace::Backtrace::force_capture();
        let report = format!(
            "Needle {} on {} {}\nThread: {thread}\nPanic: {message}\nAt: {place}\n\n{backtrace}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        error(format!("Crash in {thread}: {message} at {place}"));
        if let Some(folder) = CRASHES.get() {
            // Two crashes in the same moment still get a file each.
            static SERIAL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
            for _ in 0..8 {
                let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let name = format!("crash-{stamp}-{}-{serial}.txt", std::process::id());
                if let Ok(mut file) = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(folder.join(name))
                {
                    let _ = file.write_all(report.as_bytes());
                    break;
                }
            }
        }
        previous(info);
    }));
}

/// Take out anything that could say who the user is or what their files are called:
/// paths (with drive letters, UNC, or long-path prefixes), the user's name, and the
/// computer's name.
pub fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous = ' ';
    let starts_path = |s: &str, previous: char| -> Option<usize> {
        let b = s.as_bytes();
        if s.starts_with("\\\\") || s.starts_with("//") {
            return Some(2);
        }
        // A Unix path such as /home/ann/Music: a slash at the start of a word, then a name.
        if b.len() >= 2
            && b[0] == b'/'
            && (b[1].is_ascii_alphanumeric() || matches!(b[1], b'.' | b'_' | b'~'))
            && (previous.is_whitespace() || matches!(previous, '"' | '\'' | '(' | '=' | ':'))
        {
            return Some(1);
        }
        if b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/')
        {
            return Some(3);
        }
        None
    };
    while !rest.is_empty() {
        if starts_path(rest, previous).is_some() {
            // A path runs to the end of the line, or to a quote or bracket.
            let end = rest
                .find(['\n', '"', '\'', '<', '>', '|', ')', '('])
                .unwrap_or(rest.len());
            // Keep the names of source files: they say where the crash was.
            let path = &rest[..end];
            let name = path.rsplit(['\\', '/']).next().unwrap_or_default();
            let mut parts = name.trim_end().split(':');
            let file = parts.next().unwrap_or_default();
            let line = parts.next().unwrap_or_default();
            if file.ends_with(".rs") && !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit())
            {
                out.push_str("<src>/");
                out.push_str(name.trim_end());
            } else {
                out.push_str("<path>");
            }
            rest = &rest[end..];
            previous = '/';
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            previous = c;
            rest = &rest[c.len_utf8()..];
        }
    }
    for name in ["USERNAME", "COMPUTERNAME", "USER"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .filter(|n| n.len() >= 3)
    {
        out = out.replace(&name, "<name>");
    }
    out
}

/// Crash reports waiting to be sent, oldest first: only the `crash-*.txt` files, never the
/// list of macOS reports already kept, which lives in the same folder.
pub fn pending(data: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(crash_folder(data))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "txt"))
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-")))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

#[derive(Serialize)]
struct Report<'a> {
    version: &'a str,
    os: String,
    report: String,
}

/// What a crash report sends, exactly: shown in Settings before anything is sent.
pub fn prepare(text: &str) -> String {
    let mut report = scrub(text);
    if report.len() > MAX_REPORT {
        let mut cut = MAX_REPORT;
        while !report.is_char_boundary(cut) {
            cut -= 1;
        }
        report.truncate(cut);
        report.push_str("\n…");
    }
    report
}

/// Keeps a crash report the system made (the app's own crash, not only the core's), to be
/// sent as the core's are: cleaned of paths and names when it is sent.
pub fn keep_report(data: &Path, text: &str) -> bool {
    let folder = crash_folder(data);
    let _ = fs::create_dir_all(&folder);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
    for serial in 0..8 {
        let name = format!("crash-{stamp}-system-{serial}.txt");
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(name))
        {
            return file.write_all(text.as_bytes()).is_ok();
        }
    }
    false
}

/// A macOS crash report (`.ips`) as Needle sends it: what went wrong and where, in the
/// crashed thread, and nothing else (no Mac's identifiers, no paths, no other programs).
pub fn from_ips(ips: &str) -> Option<String> {
    let (head, body) = ips.split_once('\n')?;
    let head: serde_json::Value = serde_json::from_str(head).ok()?;
    let body: serde_json::Value = serde_json::from_str(body).ok()?;
    if head["bundleID"].as_str() != Some("fyi.nnx.Needle") {
        return None;
    }
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_string();
    let images = body["usedImages"].as_array().cloned().unwrap_or_default();
    let frame = |f: &serde_json::Value| -> String {
        let image = f["imageIndex"]
            .as_u64()
            .and_then(|i| images.get(i as usize))
            .map(|i| text(&i["name"]))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "?".into());
        match f["symbol"].as_str() {
            Some(symbol) => format!(
                "{image}  {symbol} + {}",
                f["symbolLocation"].as_u64().unwrap_or(0)
            ),
            None => format!("{image}  + 0x{:x}", f["imageOffset"].as_u64().unwrap_or(0)),
        }
    };
    let mut out = format!(
        "Needle {} on macos {} (the system's report)\nmacOS {}\n",
        text(&head["app_version"]),
        std::env::consts::ARCH,
        text(&head["os_version"]),
    );
    let exception = &body["exception"];
    out.push_str(&format!(
        "Exception: {} {} {}\n",
        text(&exception["type"]),
        text(&exception["signal"]),
        text(&exception["subtype"]),
    ));
    if let Some(indicator) = body["termination"]["indicator"].as_str() {
        out.push_str(&format!("Termination: {indicator}\n"));
    }
    // What the app said as it stopped (a Swift fatal error, an exception's reason).
    if let Some(said) = body["asi"].as_object() {
        for lines in said.values().filter_map(|v| v.as_array()) {
            for line in lines.iter().filter_map(|l| l.as_str()).take(6) {
                out.push_str(&format!("Message: {line}\n"));
            }
        }
    }
    let threads = body["threads"].as_array().cloned().unwrap_or_default();
    let crashed = body["faultingThread"].as_u64().unwrap_or(0) as usize;
    if let Some(thread) = threads.get(crashed) {
        out.push_str(&format!(
            "\nThread {crashed} {} crashed:\n",
            text(&thread["queue"])
        ));
        for (i, f) in thread["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .take(60)
            .enumerate()
        {
            out.push_str(&format!("{i:>3}  {}\n", frame(f)));
        }
    }
    if let Some(frames) = body["lastExceptionBacktrace"].as_array() {
        out.push_str("\nLast exception backtrace:\n");
        for (i, f) in frames.iter().take(40).enumerate() {
            out.push_str(&format!("{i:>3}  {}\n", frame(f)));
        }
    }
    Some(out)
}

/// macOS's own reports of Needle's crashes from the last two weeks, each kept once to be
/// sent (see [`keep_report`]). Returns how many were new.
#[cfg(target_os = "macos")]
pub fn collect_mac_crashes(data: &Path) -> usize {
    let Some(home) = std::env::var_os("HOME") else {
        return 0;
    };
    let reports = PathBuf::from(home).join("Library/Logs/DiagnosticReports");
    // Not a .txt: 1.7.1 kept this list as system-seen.txt, which was sent as a crash report
    // and then deleted, so every report was kept and sent again at the next start.
    let seen_file = crash_folder(data).join("system-seen.list");
    let old_file = crash_folder(data).join("system-seen.txt");
    let mut seen: Vec<String> = [&seen_file, &old_file]
        .iter()
        .flat_map(|f| fs::read_to_string(f).unwrap_or_default().lines().map(str::to_string).collect::<Vec<_>>())
        .collect();
    let _ = fs::remove_file(&old_file);
    let mut new = 0;
    for entry in fs::read_dir(&reports).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("Needle-") && name.ends_with(".ips")) || seen.contains(&name) {
            continue;
        }
        let recent = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < std::time::Duration::from_secs(14 * 86_400));
        if recent
            && let Ok(ips) = fs::read_to_string(entry.path())
            && let Some(report) = from_ips(&ips)
            && keep_report(data, &report)
        {
            new += 1;
        }
        seen.push(name);
    }
    // The names only, so each report is kept once; the list stays short.
    let keep = seen.len().saturating_sub(200);
    let _ = fs::write(&seen_file, seen[keep..].join("\n"));
    if new > 0 {
        info(format!("Kept {new} crash reports from macOS"));
    }
    new
}

/// Send waiting crash reports when `send` is true; delete sent ones and ones older than
/// 30 days. Returns how many were sent.
pub fn send_pending(data: &Path, send: bool) -> usize {
    let mut sent = 0;
    for file in pending(data) {
        let old = fs::metadata(&file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > KEEP_UNSENT);
        // Reports older than 30 days are never sent.
        if send
            && !old
            && let Ok(text) = fs::read_to_string(&file)
            && post(&text).is_ok()
        {
            let _ = fs::remove_file(&file);
            sent += 1;
            continue;
        }
        if old {
            let _ = fs::remove_file(&file);
        }
    }
    if sent > 0 {
        info(format!("Sent {sent} crash reports"));
    }
    sent
}

/// The request body, kept under the site's 32 KB limit even after JSON escaping.
fn crash_json(text: &str) -> Result<String> {
    let mut report = prepare(text);
    loop {
        let body = serde_json::to_string(&Report {
            version: env!("CARGO_PKG_VERSION"),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            report: report.clone(),
        })?;
        if body.len() <= 30 * 1024 {
            return Ok(body);
        }
        let mut cut = report.len() * 3 / 4;
        while !report.is_char_boundary(cut) {
            cut -= 1;
        }
        report.truncate(cut);
    }
}

fn post(text: &str) -> Result<()> {
    crate::integrations::client()?
        .post(CRASH_URL)
        .header("Content-Type", "application/json")
        .body(crash_json(text)?)
        .timeout(Duration::from_secs(15))
        .send()?
        .error_for_status()?;
    Ok(())
}

/// The last part of the log, for "Copy error report".
pub fn recent(data: &Path, lines: usize) -> String {
    let text = fs::read_to_string(log_folder(data).join("needle.log")).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macos_report_keeps_what_went_wrong_and_where_only() {
        let head = r#"{"app_name":"Needle","app_version":"1.7.0","bundleID":"fyi.nnx.Needle","os_version":"macOS 26.1 (25B78)","name":"Needle"}"#;
        let body = r#"{"crashReporterKey":"SECRET-KEY","modelCode":"Mac16,12","userID":501,
            "procPath":"/Users/someone/Applications/Needle.app/Contents/MacOS/Needle",
            "exception":{"type":"EXC_BREAKPOINT","signal":"SIGTRAP"},
            "termination":{"indicator":"Trace/BPT trap: 5"},
            "asi":{"libswiftCore.dylib":["Needle/Store.swift:120: Fatal error: Index out of range"]},
            "faultingThread":0,
            "threads":[{"queue":"com.apple.main-thread","frames":[
                {"imageIndex":1,"imageOffset":4096,"symbol":"Store.tick()","symbolLocation":44},
                {"imageIndex":0,"imageOffset":8192}]}],
            "usedImages":[{"name":"Needle","path":"/Users/someone/Applications/Needle.app/Contents/MacOS/Needle"},
                {"name":"libswiftCore.dylib","path":"/usr/lib/swift/libswiftCore.dylib"}]}"#;
        let report = from_ips(&format!("{head}\n{body}")).unwrap();
        assert!(report.starts_with("Needle 1.7.0 on macos"));
        assert!(report.contains("EXC_BREAKPOINT SIGTRAP"));
        assert!(report.contains("Fatal error: Index out of range"));
        assert!(report.contains("libswiftCore.dylib  Store.tick() + 44"));
        assert!(report.contains("Needle  + 0x2000"));
        for private in ["SECRET-KEY", "Mac16,12", "someone", "501"] {
            assert!(!report.contains(private), "{private} in {report}");
        }
        // Another program's report is not Needle's.
        assert!(
            from_ips(&format!(
                "{}\n{body}",
                head.replace("fyi.nnx.Needle", "com.other.App")
            ))
            .is_none()
        );
    }

    #[test]
    fn scrubbing_removes_paths_and_names_but_keeps_source_places() {
        let text = "Cannot open C:\\Users\\Ann\\Music\\Secret Song.flac\nnot found: \\\\nas\\share\\x.mp3 (os error 2)\nAt: crates\\needle-core\\src\\audio.rs:120\n at C:\\Users\\build\\.cargo\\registry\\src\\rodio-0.21\\src\\sink.rs:40";
        let out = scrub(text);
        let unix =
            scrub("open /home/alice/Music/private.flac: no\n\"/Users/bob/x.mp3\" 24/96 kHz and/or");
        assert!(!unix.contains("alice") && !unix.contains("bob"), "{unix}");
        assert!(unix.contains("24/96 kHz and/or"), "{unix}");
        let named = scrub("C:\\Users\\ann\\private-playlist.rs could not be read");
        assert!(!named.contains("private-playlist"), "{named}");
        assert!(!out.contains("Secret Song"), "{out}");
        assert!(!out.contains("nas\\share"), "{out}");
        assert!(out.contains("<path>"));
        assert!(out.contains("audio.rs:120"));
        assert!(out.contains("sink.rs:40"));
    }

    #[test]
    fn a_crash_leaves_a_report_and_the_log_says_so() {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path());
        info("hello from the test");
        let _ = std::thread::Builder::new()
            .name("crasher".into())
            .spawn(|| panic!("the test crashed on purpose"))
            .unwrap()
            .join();
        let _ = std::thread::Builder::new()
            .name("crasher".into())
            .spawn(|| panic!("the test crashed on purpose"))
            .unwrap()
            .join();
        let reports = pending(dir.path());
        assert_eq!(reports.len(), 2, "two crashes, two reports: {reports:?}");
        let report = fs::read_to_string(&reports[0]).unwrap();
        assert!(
            report.contains("Panic: the test crashed on purpose"),
            "{report}"
        );
        assert!(report.contains("Thread: crasher"));
        assert!(report.contains("logfile.rs"));
        let log = recent(dir.path(), 10);
        assert!(log.contains("hello from the test"));
        assert!(log.contains("Crash in crasher"));
    }

    #[test]
    fn crash_reports_wait_and_old_ones_are_cleared() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(crash_folder(dir.path())).unwrap();
        fs::write(crash_folder(dir.path()).join("crash-1.txt"), "Panic: x").unwrap();
        assert_eq!(pending(dir.path()).len(), 1);
        // With reports off, a new report stays until it is 30 days old.
        assert_eq!(send_pending(dir.path(), false), 0);
        assert_eq!(pending(dir.path()).len(), 1);
        // The list of macOS reports already kept is never a report.
        fs::write(crash_folder(dir.path()).join("system-seen.list"), "Needle-1.ips").unwrap();
        fs::write(crash_folder(dir.path()).join("system-seen.txt"), "Needle-1.ips").unwrap();
        assert_eq!(pending(dir.path()).len(), 1);
        let long = "a".repeat(MAX_REPORT * 2);
        assert!(prepare(&long).len() <= MAX_REPORT + 4);
        // Quotes and backslashes double in JSON; the body still fits.
        let escapes = "\"\\\n".repeat(MAX_REPORT);
        assert!(crash_json(&escapes).unwrap().len() <= 30 * 1024);
    }
}
