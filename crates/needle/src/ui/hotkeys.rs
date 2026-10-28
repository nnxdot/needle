//! Keys that work everywhere, even with Needle in the background (Settings › Keyboard ›
//! Shortcuts everywhere). Windows: the system's hot keys (`RegisterHotKey`). Linux with X11:
//! the keys are grabbed on the root window. Wayland lets no app take keys for itself; there the
//! desktop's own keyboard settings bind keys (Needle answers the media keys and `playerctl`).
use std::sync::{Arc, Mutex};

/// Every action a key can do, with its name in Settings, in the order shown.
pub const ACTIONS: [(&str, &str); 7] = [
    ("play", "Play / Pause"),
    ("next", "Next song"),
    ("previous", "Previous song"),
    ("favorite", "Favorite this song"),
    ("mini", "Show the small player"),
    ("volume_up", "Volume up"),
    ("volume_down", "Volume down"),
];

/// Keys pressed together: the modifiers and one key, as "ctrl-alt-space".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// "a"–"z", "0"–"9", "f1"–"f24", "space", "left", "right", "up", "down", "home", "end",
    /// "pageup", "pagedown", "insert", "delete", "enter", "tab", "backspace", or a sign
    /// ("-", "=", ",", ".", "/", ";", "'", "[", "]", "\\", "`").
    pub key: String,
}

const NAMED: [&str; 14] = [
    "space",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "insert",
    "delete",
    "enter",
    "tab",
    "backspace",
];
const SIGNS: [&str; 11] = ["-", "=", ",", ".", "/", ";", "'", "[", "]", "\\", "`"];

fn known_key(key: &str) -> bool {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_lowercase() || c.is_ascii_digit() || SIGNS.contains(&key),
        _ => {
            NAMED.contains(&key)
                || key
                    .strip_prefix('f')
                    .and_then(|n| n.parse::<u8>().ok())
                    .is_some_and(|n| (1..=24).contains(&n))
        }
    }
}

impl Chord {
    /// "ctrl-alt-space" → the chord. None for keys Needle cannot take, or a letter alone
    /// (a key everywhere needs Ctrl, Alt, or the Windows key, unless it is an F key).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_lowercase();
        // The last part is the key ("-" itself may be the key: "ctrl--").
        let (mods, key) = match text.strip_suffix("--") {
            Some(rest) => (rest, "-".to_string()),
            None => {
                let (mods, key) = text.rsplit_once('-').unwrap_or(("", &text));
                (mods, key.to_string())
            }
        };
        let mut chord = Chord {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key,
        };
        for part in mods.split('-').filter(|p| !p.is_empty()) {
            match part {
                "ctrl" | "control" => chord.ctrl = true,
                "alt" => chord.alt = true,
                "shift" => chord.shift = true,
                "win" | "super" | "cmd" | "platform" => chord.win = true,
                _ => return None,
            }
        }
        let f_key = chord.key.starts_with('f') && chord.key.len() > 1;
        (known_key(&chord.key) && (chord.ctrl || chord.alt || chord.win || f_key)).then_some(chord)
    }

    pub fn text(&self) -> String {
        let mut parts = vec![];
        for (on, name) in [
            (self.ctrl, "ctrl"),
            (self.alt, "alt"),
            (self.shift, "shift"),
            (self.win, "win"),
        ] {
            if on {
                parts.push(name.to_string());
            }
        }
        parts.push(self.key.clone());
        parts.join("-")
    }

    /// The key caps to show: ["Ctrl", "Alt", "Space"], with arrows as ← → ↑ ↓.
    pub fn caps(&self) -> Vec<String> {
        let mut caps = vec![];
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.win, if cfg!(windows) { "Win" } else { "Super" }),
        ] {
            if on {
                caps.push(name.to_string());
            }
        }
        caps.push(match self.key.as_str() {
            "left" => "←".into(),
            "right" => "→".into(),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "pageup" => "Page Up".into(),
            "pagedown" => "Page Down".into(),
            "backspace" => "Backspace".into(),
            key if key.len() == 1 => key.to_uppercase(),
            key if key.starts_with('f') => key.to_uppercase(),
            key => {
                let mut c = key.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
        });
        caps
    }
}

/// What the keys are doing: the ones another app holds (by action), and whether this system
/// can take keys at all.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub taken: Vec<String>,
    pub supported: bool,
}

/// The keys, held by a thread of their own; actions arrive through `pressed`.
pub struct Hotkeys {
    status: Arc<Mutex<Status>>,
    #[cfg(windows)]
    thread: Option<u32>,
    #[cfg(windows)]
    wanted: Arc<Mutex<Vec<(String, Chord)>>>,
    #[cfg(target_os = "linux")]
    linux: Option<linux::Grabs>,
}

impl Hotkeys {
    /// Start, with `pressed` called (on the keys' thread) with the action of a key pressed.
    pub fn start(pressed: impl Fn(String) + Send + 'static) -> Self {
        let status = Arc::new(Mutex::new(Status::default()));
        #[cfg(windows)]
        {
            let wanted = Arc::new(Mutex::new(vec![]));
            let thread = windows::start(wanted.clone(), status.clone(), pressed);
            Self {
                status,
                thread,
                wanted,
            }
        }
        #[cfg(target_os = "linux")]
        {
            let linux = linux::Grabs::start(status.clone(), pressed);
            Self { status, linux }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = pressed;
            Self { status }
        }
    }

    /// Hold these keys (action, "ctrl-alt-space"), letting go of any others.
    pub fn set(&self, keys: &[(String, String)]) {
        let chords: Vec<(String, Chord)> = keys
            .iter()
            .filter_map(|(action, text)| Some((action.clone(), Chord::parse(text)?)))
            .collect();
        #[cfg(windows)]
        {
            *self.wanted.lock().unwrap_or_else(|e| e.into_inner()) = chords;
            if let Some(thread) = self.thread {
                windows::wake(thread);
            }
        }
        #[cfg(target_os = "linux")]
        if let Some(linux) = &self.linux {
            linux.set(chords);
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        let _ = chords;
    }

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg(windows)]
mod windows {
    use super::{Chord, Status};
    use std::sync::{Arc, Mutex};
    use windows_sys::Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
                UnregisterHotKey,
            },
            WindowsAndMessaging::{GetMessageW, MSG, PostThreadMessageW, WM_APP, WM_HOTKEY},
        },
    };

    const WM_KEYS_CHANGED: u32 = WM_APP + 41;

    /// The virtual-key code for a key name.
    fn virtual_key(key: &str) -> Option<u32> {
        Some(match key {
            "space" => 0x20,
            "pageup" => 0x21,
            "pagedown" => 0x22,
            "end" => 0x23,
            "home" => 0x24,
            "left" => 0x25,
            "up" => 0x26,
            "right" => 0x27,
            "down" => 0x28,
            "insert" => 0x2D,
            "delete" => 0x2E,
            "enter" => 0x0D,
            "tab" => 0x09,
            "backspace" => 0x08,
            ";" => 0xBA,
            "=" => 0xBB,
            "," => 0xBC,
            "-" => 0xBD,
            "." => 0xBE,
            "/" => 0xBF,
            "`" => 0xC0,
            "[" => 0xDB,
            "\\" => 0xDC,
            "]" => 0xDD,
            "'" => 0xDE,
            k if k.len() == 1 => k.chars().next()?.to_ascii_uppercase() as u32,
            k => 0x6F + k.strip_prefix('f')?.parse::<u32>().ok()?,
        })
    }

    pub fn start(
        wanted: Arc<Mutex<Vec<(String, Chord)>>>,
        status: Arc<Mutex<Status>>,
        pressed: impl Fn(String) + Send + 'static,
    ) -> Option<u32> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("needle-hotkeys".into())
            .spawn(move || unsafe {
                let _ = tx.send(GetCurrentThreadId());
                status.lock().unwrap_or_else(|e| e.into_inner()).supported = true;
                let mut held: Vec<String> = vec![];
                let mut msg: MSG = std::mem::zeroed();
                // The first message makes the queue; the keys are set when the app sends them.
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    match msg.message {
                        WM_HOTKEY => {
                            if let Some(action) = held.get(msg.wParam) {
                                pressed(action.clone());
                            }
                        }
                        WM_KEYS_CHANGED => {
                            for id in 0..held.len() {
                                UnregisterHotKey(0, id as i32);
                            }
                            held.clear();
                            let mut taken = vec![];
                            for (action, chord) in
                                wanted.lock().unwrap_or_else(|e| e.into_inner()).iter()
                            {
                                let Some(key) = virtual_key(&chord.key) else {
                                    continue;
                                };
                                let mut mods = MOD_NOREPEAT;
                                for (on, m) in [
                                    (chord.ctrl, MOD_CONTROL),
                                    (chord.alt, MOD_ALT),
                                    (chord.shift, MOD_SHIFT),
                                    (chord.win, MOD_WIN),
                                ] {
                                    if on {
                                        mods |= m;
                                    }
                                }
                                let id = held.len() as i32;
                                if RegisterHotKey(0, id, mods, key) != 0 {
                                    held.push(action.clone());
                                } else {
                                    // Another app holds these keys; keep the place empty.
                                    taken.push(action.clone());
                                    held.push(String::new());
                                }
                            }
                            status.lock().unwrap_or_else(|e| e.into_inner()).taken = taken;
                        }
                        _ => {}
                    }
                }
            })
            .ok()?;
        rx.recv().ok()
    }

    pub fn wake(thread: u32) {
        // The thread may not have its message queue yet: try again for a moment.
        std::thread::spawn(move || {
            for _ in 0..50 {
                if unsafe { PostThreadMessageW(thread, WM_KEYS_CHANGED, 0, 0) } != 0 {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        });
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{Chord, Status};
    use std::sync::{Arc, Mutex, mpsc};
    use x11rb::{
        connection::Connection,
        protocol::{
            Event,
            xproto::{ConnectionExt, GrabMode, Keycode, ModMask},
        },
        rust_connection::RustConnection,
    };

    /// The X keysym for a key name.
    fn keysym(key: &str) -> Option<u32> {
        Some(match key {
            "space" => 0x20,
            "left" => 0xff51,
            "up" => 0xff52,
            "right" => 0xff53,
            "down" => 0xff54,
            "pageup" => 0xff55,
            "pagedown" => 0xff56,
            "end" => 0xff57,
            "home" => 0xff50,
            "insert" => 0xff63,
            "delete" => 0xffff,
            "enter" => 0xff0d,
            "tab" => 0xff09,
            "backspace" => 0xff08,
            "-" => 0x2d,
            "=" => 0x3d,
            "," => 0x2c,
            "." => 0x2e,
            "/" => 0x2f,
            ";" => 0x3b,
            "'" => 0x27,
            "[" => 0x5b,
            "]" => 0x5d,
            "\\" => 0x5c,
            "`" => 0x60,
            k if k.len() == 1 => k.chars().next()? as u32,
            k => 0xffbd + k.strip_prefix('f')?.parse::<u32>().ok()?,
        })
    }

    pub struct Grabs {
        tx: mpsc::Sender<Vec<(String, Chord)>>,
    }

    impl Grabs {
        /// Only on X11: without a display (Wayland alone), there is nothing to grab.
        pub fn start(
            status: Arc<Mutex<Status>>,
            pressed: impl Fn(String) + Send + 'static,
        ) -> Option<Self> {
            if std::env::var_os("DISPLAY").is_none()
                || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland")
            {
                return None;
            }
            let (connection, screen) = x11rb::connect(None).ok()?;
            status.lock().unwrap_or_else(|e| e.into_inner()).supported = true;
            let (tx, rx) = mpsc::channel::<Vec<(String, Chord)>>();
            std::thread::Builder::new()
                .name("needle-hotkeys".into())
                .spawn(move || run(connection, screen, rx, status, pressed))
                .ok()?;
            Some(Self { tx })
        }
        pub fn set(&self, chords: Vec<(String, Chord)>) {
            let _ = self.tx.send(chords);
        }
    }

    fn run(
        connection: RustConnection,
        screen: usize,
        rx: mpsc::Receiver<Vec<(String, Chord)>>,
        status: Arc<Mutex<Status>>,
        pressed: impl Fn(String),
    ) {
        let root = connection.setup().roots[screen].root;
        let (min, max) = (
            connection.setup().min_keycode,
            connection.setup().max_keycode,
        );
        let Ok(Ok(map)) = connection
            .get_keyboard_mapping(min, max - min + 1)
            .map(|c| c.reply())
        else {
            return;
        };
        let per = map.keysyms_per_keycode as usize;
        let keycode = |sym: u32| -> Option<Keycode> {
            map.keysyms
                .chunks(per.max(1))
                .position(|syms| syms.contains(&sym))
                .map(|i| min + i as u8)
        };
        // Num Lock and Caps Lock change the modifiers a key comes with: grab with each.
        let locks = [
            ModMask::from(0u16),
            ModMask::M2,
            ModMask::LOCK,
            ModMask::M2 | ModMask::LOCK,
        ];
        let mut held: Vec<(Keycode, u16, String)> = vec![];
        loop {
            if let Ok(chords) = rx.try_recv() {
                for (code, mods, _) in held.drain(..) {
                    for lock in locks {
                        let _ = connection.ungrab_key(code, root, ModMask::from(mods) | lock);
                    }
                }
                let mut taken = vec![];
                for (action, chord) in chords {
                    let Some(code) = keysym(&chord.key).and_then(keycode) else {
                        continue;
                    };
                    let mut mods = 0u16;
                    for (on, m) in [
                        (chord.ctrl, ModMask::CONTROL),
                        (chord.alt, ModMask::M1),
                        (chord.shift, ModMask::SHIFT),
                        (chord.win, ModMask::M4),
                    ] {
                        if on {
                            mods |= u16::from(m);
                        }
                    }
                    let mut ok = true;
                    for lock in locks {
                        let grab = connection.grab_key(
                            true,
                            root,
                            ModMask::from(mods) | lock,
                            code,
                            GrabMode::ASYNC,
                            GrabMode::ASYNC,
                        );
                        // Another client holding the keys answers with an error.
                        ok &= grab.is_ok_and(|c| c.check().is_ok());
                    }
                    if ok {
                        held.push((code, mods, action));
                    } else {
                        taken.push(action);
                    }
                }
                let _ = connection.flush();
                status.lock().unwrap_or_else(|e| e.into_inner()).taken = taken;
            }
            match connection.poll_for_event() {
                Ok(Some(Event::KeyPress(event))) => {
                    let mods = u16::from(event.state)
                        & !(u16::from(ModMask::M2) | u16::from(ModMask::LOCK));
                    let mods = mods
                        & (u16::from(ModMask::CONTROL)
                            | u16::from(ModMask::M1)
                            | u16::from(ModMask::SHIFT)
                            | u16::from(ModMask::M4));
                    if let Some((_, _, action)) = held
                        .iter()
                        .find(|(code, m, _)| *code == event.detail && *m == mods)
                    {
                        pressed(action.clone());
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(15)),
                Err(_) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Chord;

    #[test]
    fn chords_read_and_write() {
        let c = Chord::parse("Ctrl-Alt-Space").unwrap();
        assert!(c.ctrl && c.alt && !c.shift);
        assert_eq!(c.text(), "ctrl-alt-space");
        assert_eq!(c.caps(), vec!["Ctrl", "Alt", "Space"]);
        assert_eq!(
            Chord::parse("ctrl-alt-right").unwrap().caps(),
            vec!["Ctrl", "Alt", "→"]
        );
        assert_eq!(Chord::parse("ctrl--").unwrap().key, "-");
        assert_eq!(Chord::parse("f8").unwrap().text(), "f8");
        // A letter alone would take the key from every app.
        assert!(Chord::parse("a").is_none());
        assert!(Chord::parse("shift-a").is_none());
        assert!(Chord::parse("ctrl-hyper").is_none());
        assert!(Chord::parse("ctrl-f25").is_none());
    }
}
