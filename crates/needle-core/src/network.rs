//! Whether the connection costs: metered, as mobile data, a phone's hotspot, or Low Data Mode.
//! The phones and the Mac tell the core (`set_metered`); on Windows and Linux the core asks the
//! system itself (`watch`). With Settings › Fewer lookups on metered connections on (the
//! default), background lookups (the search for artists' pictures) wait for a free network.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static METERED: AtomicBool = AtomicBool::new(false);
static WATCHING: AtomicBool = AtomicBool::new(false);

type Listener = Box<dyn Fn() + Send + Sync>;
static LISTENERS: Mutex<Vec<Listener>> = Mutex::new(Vec::new());

/// Call `listener` when a network may have come back: an app tells of its network (it does at
/// each change), or the one in use stops being metered. Things waiting for a server (playlist
/// changes) try again then.
pub fn on_change(listener: impl Fn() + Send + Sync + 'static) {
    LISTENERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(listener));
}

fn changed() {
    for listener in LISTENERS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        listener();
    }
}

/// The app says whether the network it uses now is metered. Apps say it at each change of
/// network, so it also tells that a network is there.
pub fn set_metered(on: bool) {
    METERED.store(on, Ordering::SeqCst);
    changed();
}

/// Whether the network in use now is metered, as far as Needle knows.
pub fn metered() -> bool {
    METERED.load(Ordering::SeqCst)
}

/// Background lookups may run now: not on a metered network while `fewer_on_metered` (Settings ›
/// Fewer lookups on metered connections) is on.
pub fn background_lookups_allowed(fewer_on_metered: bool) -> bool {
    !(fewer_on_metered && metered())
}

/// Asks the system now and every 30 seconds whether the network is metered (Windows, Linux).
/// Once per run; elsewhere the app says it (`set_metered`).
pub fn watch() {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("metered".into())
        .spawn(|| {
            loop {
                if let Some(on) = system_metered()
                    && METERED.swap(on, Ordering::SeqCst)
                    && !on
                {
                    changed();
                }
                std::thread::sleep(Duration::from_secs(30));
            }
        });
}

/// Windows: the connection's cost as Windows sees it (set per network in Settings › Network,
/// and on by itself for mobile data and phone hotspots).
#[cfg(windows)]
fn system_metered() -> Option<bool> {
    use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};
    let profile = NetworkInformation::GetInternetConnectionProfile().ok()?;
    let cost = profile.GetConnectionCost().ok()?;
    let kind = cost.NetworkCostType().ok()?;
    let costly = kind == NetworkCostType::Fixed || kind == NetworkCostType::Variable;
    Some(
        costly
            || cost.Roaming().unwrap_or(false)
            || cost.OverDataLimit().unwrap_or(false)
            || cost.ApproachingDataLimit().unwrap_or(false),
    )
}

/// Linux: NetworkManager's own answer (1 yes, 3 guessed yes), when it runs.
#[cfg(target_os = "linux")]
fn system_metered() -> Option<bool> {
    let output = std::process::Command::new("busctl")
        .args([
            "--system",
            "get-property",
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "Metered",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_networkmanager(&String::from_utf8_lossy(&output.stdout))
}

/// NetworkManager's Metered property as busctl prints it ("u 1").
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_networkmanager(text: &str) -> Option<bool> {
    let value: u32 = text.split_whitespace().nth(1)?.parse().ok()?;
    Some(value == 1 || value == 3)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_metered() -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networkmanager_says_metered() {
        assert_eq!(parse_networkmanager("u 1\n"), Some(true));
        assert_eq!(parse_networkmanager("u 3"), Some(true));
        assert_eq!(parse_networkmanager("u 2"), Some(false));
        assert_eq!(parse_networkmanager("u 4"), Some(false));
        assert_eq!(parse_networkmanager("nonsense"), None);
    }

    /// Windows answers whether this computer's network is metered (it may not, offline).
    #[cfg(windows)]
    #[test]
    fn windows_can_be_asked() {
        println!("metered here: {:?}", system_metered());
    }

    #[test]
    fn lookups_wait_on_a_metered_network_only_when_asked() {
        set_metered(true);
        assert!(!background_lookups_allowed(true));
        assert!(background_lookups_allowed(false));
        set_metered(false);
        assert!(background_lookups_allowed(true));
    }
}
