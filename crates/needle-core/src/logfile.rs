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
            let name = format!(
                "crash-{}.txt",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%3f")
            );
            let _ = fs::write(folder.join(name), &report);
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
    let starts_path = |s: &str| -> Option<usize> {
        let b = s.as_bytes();
        if s.starts_with("\\\\") || s.starts_with("//") {
            return Some(2);
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
        if starts_path(rest).is_some() {
            // A path runs to the end of the line, or to a quote or bracket.
            let end = rest
                .find(['\n', '"', '\'', '<', '>', '|', ')', '('])
                .unwrap_or(rest.len());
            // Keep the names of source files: they say where the crash was.
            let path = &rest[..end];
            let name = path.rsplit(['\\', '/']).next().unwrap_or_default();
            let file = name.split(':').next().unwrap_or_default();
            if file.ends_with(".rs") {
                out.push_str("<src>/");
                out.push_str(name.trim_end());
            } else {
                out.push_str("<path>");
            }
            rest = &rest[end..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
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

/// Crash reports waiting to be sent, oldest first.
pub fn pending(data: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(crash_folder(data))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "txt"))
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
        if send
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

fn post(text: &str) -> Result<()> {
    let body = Report {
        version: env!("CARGO_PKG_VERSION"),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        report: prepare(text),
    };
    crate::integrations::client()?
        .post(CRASH_URL)
        .json(&body)
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
    fn scrubbing_removes_paths_and_names_but_keeps_source_places() {
        let text = "Cannot open C:\\Users\\Ann\\Music\\Secret Song.flac\nnot found: \\\\nas\\share\\x.mp3 (os error 2)\nAt: crates\\needle-core\\src\\audio.rs:120\n at C:\\Users\\build\\.cargo\\registry\\src\\rodio-0.21\\src\\sink.rs:40";
        let out = scrub(text);
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
        let reports = pending(dir.path());
        assert_eq!(reports.len(), 1, "{reports:?}");
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
        let long = "a".repeat(MAX_REPORT * 2);
        assert!(prepare(&long).len() <= MAX_REPORT + 4);
    }
}
