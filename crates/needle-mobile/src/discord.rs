//! Discord's "Listening to" status from the phone. Discord's own library (its Social SDK)
//! hands the status to the Discord app on the phone; there is no sign-in. The library is not
//! part of Needle's source, so the app loads it when the build has it (see MOBILE.md), then
//! calls [`Needle::start_discord`].
use crate::{Needle, Result};

/// The Discord application Needle appears as, as on desktop.
#[cfg(target_os = "android")]
const APP_ID: &str = "1552097597454295110";

/// The Discord status settings, as on desktop.
#[derive(Clone, uniffi::Record)]
pub struct DiscordSettings {
    pub on: bool,
    /// Look up covers in Apple's iTunes catalog (artist and song name only).
    pub covers: bool,
    pub logo: bool,
    /// Keep the song on the profile, marked Paused, while paused.
    pub paused: bool,
    /// Minutes of nothing playing before the status goes away; 0 keeps it.
    pub idle_minutes: u32,
    /// "Listening to …", then three lines: "song", "artist", "album", "needle", or "none".
    pub title: String,
    pub top: String,
    pub middle: String,
    pub bottom: String,
}

#[uniffi::export]
impl Needle {
    pub fn discord_settings(&self) -> Result<DiscordSettings> {
        let s = self.library.settings()?;
        Ok(DiscordSettings {
            on: s.discord_presence,
            covers: s.discord_covers,
            logo: s.discord_logo,
            paused: s.discord_paused,
            idle_minutes: s.discord_idle_minutes,
            title: s.discord_title,
            top: s.discord_top,
            middle: s.discord_middle,
            bottom: s.discord_bottom,
        })
    }

    pub fn set_discord_settings(&self, value: DiscordSettings) -> Result<()> {
        self.library.update_settings(|s| {
            s.discord_presence = value.on;
            s.discord_covers = value.covers;
            s.discord_logo = value.logo;
            s.discord_paused = value.paused;
            s.discord_idle_minutes = value.idle_minutes;
            s.discord_title = value.title;
            s.discord_top = value.top;
            s.discord_middle = value.middle;
            s.discord_bottom = value.bottom;
        })?;
        Ok(())
    }

    /// Keeps the Discord status in step with playback, from now on (when it is on in
    /// Settings). Call once, after the app has loaded Discord's library.
    pub fn start_discord(&self) {
        #[cfg(target_os = "android")]
        {
            use std::sync::atomic::{AtomicBool, Ordering};
            static STARTED: AtomicBool = AtomicBool::new(false);
            if STARTED.swap(true, Ordering::SeqCst) {
                return;
            }
            let (player, library) = (self.player.clone(), self.library.clone());
            std::thread::spawn(move || follow(player, library));
        }
    }
}

/// As on desktop: tells Discord when the song, pause, or position changes, and clears the
/// status after a while of nothing playing.
#[cfg(target_os = "android")]
fn follow(player: needle_core::audio::Player, library: needle_core::database::Library) {
    use needle_core::discord::{Activity, Layout, Presence};
    let mut presence: Option<Presence> = None;
    let mut sent: Option<(_, i64)> = None;
    let mut idle_since: Option<i64> = None;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let Ok(settings) = library.settings() else {
            continue;
        };
        if !settings.discord_presence {
            presence = None;
            sent = None;
            continue;
        }
        let state = player.state();
        // A server song that is still connecting has no start time yet.
        if state.loading {
            continue;
        }
        let presence =
            presence.get_or_insert_with(|| Presence::start_with::<sdk::Sdk>(APP_ID.into()));
        let now = chrono::Utc::now().timestamp();
        if state.playing {
            idle_since = None;
        } else if idle_since.is_none() {
            idle_since = Some(now);
        }
        let idle = settings.discord_idle_minutes > 0
            && idle_since
                .is_some_and(|since| now - since >= settings.discord_idle_minutes as i64 * 60);
        let hidden = idle || (!state.playing && !settings.discord_paused);
        let layout = Layout::from_settings(&settings);
        let activity = state.current.as_ref().filter(|_| !hidden).map(|item| {
            let track = &item.track;
            let started = now - state.position.round() as i64;
            Activity {
                title: track.title.clone(),
                artist: track.display_artist().to_string(),
                album: track.album.clone(),
                started,
                ends: (track.duration > 0.).then(|| started + track.duration.round() as i64),
                paused: !state.playing,
                find_cover: settings.discord_covers,
                cover: None,
                layout,
            }
        });
        // What Discord shows; a change of settings also counts.
        let key = activity.as_ref().map(|a| {
            (
                (
                    a.title.clone(),
                    a.artist.clone(),
                    a.paused,
                    a.find_cover,
                    a.layout,
                ),
                a.started,
            )
        });
        let changed = match (&sent, &key) {
            (None, None) => false,
            (Some((what, started)), Some((new_what, new_started))) => {
                what != new_what || (!new_what.2 && (started - new_started).abs() > 2i64)
            }
            _ => true,
        };
        if changed {
            presence.set(activity);
            sent = key;
        }
    }
}

/// Discord's library, called through its C interface. It is loaded by the app (Java needs it
/// first), so here it is only looked up.
#[cfg(target_os = "android")]
mod sdk {
    use needle_core::discord::{Activity, Link};
    use std::{
        ffi::{c_char, c_int, c_void},
        ptr::null_mut,
        sync::{Arc, Mutex, OnceLock},
        time::{Duration, Instant},
    };

    /// Discord does not find Needle's logo by its name when the status comes from a phone, so
    /// it gets the logo's web link.
    const LOGO: &str = "https://needle.nnx.fyi/icon-512.png";

    /// Every object of Discord's C interface: one hidden pointer.
    #[repr(C)]
    struct Object {
        opaque: *mut c_void,
    }

    impl Object {
        fn new() -> Self {
            Self { opaque: null_mut() }
        }
    }

    #[repr(C)]
    struct Text {
        ptr: *mut u8,
        size: usize,
    }

    impl Text {
        /// Discord copies what it is given, so a borrowed string will do.
        fn of(s: &str) -> Self {
            Self {
                ptr: s.as_ptr().cast_mut(),
                size: s.len(),
            }
        }
    }

    type Free = unsafe extern "C" fn(*mut c_void);
    type Answered = unsafe extern "C" fn(*mut Object, *mut c_void);
    type Logged = unsafe extern "C" fn(Text, c_int, *mut c_void);

    unsafe extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    macro_rules! api {
        ($($field:ident = $name:literal: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
            struct Api {
                $($field: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
            }

            impl Api {
                fn load() -> Option<Self> {
                    // SAFETY: looks up a library the app has loaded, and functions with the
                    // types Discord's header (cdiscord.h) gives them.
                    unsafe {
                        let library = dlopen(c"libdiscord_partner_sdk.so".as_ptr(), RTLD_NOW);
                        if library.is_null() {
                            return None;
                        }
                        Some(Self {$(
                            $field: {
                                let f = dlsym(library, concat!($name, "\0").as_ptr().cast());
                                if f.is_null() {
                                    return None;
                                }
                                std::mem::transmute::<*mut c_void, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(f)
                            },
                        )*})
                    }
                }
            }
        };
    }

    api! {
        run_callbacks = "Discord_RunCallbacks": fn();
        free = "Discord_Free": fn(*mut c_void);
        client_init = "Discord_Client_Init": fn(*mut Object);
        client_drop = "Discord_Client_Drop": fn(*mut Object);
        set_application_id = "Discord_Client_SetApplicationId": fn(*mut Object, u64);
        add_log_callback = "Discord_Client_AddLogCallback": fn(*mut Object, Logged, Option<Free>, *mut c_void, c_int);
        update_rich_presence = "Discord_Client_UpdateRichPresence": fn(*mut Object, *mut Object, Answered, Option<Free>, *mut c_void);
        clear_rich_presence = "Discord_Client_ClearRichPresence": fn(*mut Object);
        activity_init = "Discord_Activity_Init": fn(*mut Object);
        activity_drop = "Discord_Activity_Drop": fn(*mut Object);
        activity_set_name = "Discord_Activity_SetName": fn(*mut Object, Text);
        activity_set_type = "Discord_Activity_SetType": fn(*mut Object, c_int);
        activity_set_display = "Discord_Activity_SetStatusDisplayType": fn(*mut Object, *const c_int);
        activity_set_details = "Discord_Activity_SetDetails": fn(*mut Object, *const Text);
        activity_set_state = "Discord_Activity_SetState": fn(*mut Object, *const Text);
        activity_set_assets = "Discord_Activity_SetAssets": fn(*mut Object, *mut Object);
        activity_set_timestamps = "Discord_Activity_SetTimestamps": fn(*mut Object, *mut Object);
        assets_init = "Discord_ActivityAssets_Init": fn(*mut Object);
        assets_drop = "Discord_ActivityAssets_Drop": fn(*mut Object);
        assets_set_large_image = "Discord_ActivityAssets_SetLargeImage": fn(*mut Object, *const Text);
        assets_set_large_text = "Discord_ActivityAssets_SetLargeText": fn(*mut Object, *const Text);
        assets_set_small_image = "Discord_ActivityAssets_SetSmallImage": fn(*mut Object, *const Text);
        assets_set_small_text = "Discord_ActivityAssets_SetSmallText": fn(*mut Object, *const Text);
        times_init = "Discord_ActivityTimestamps_Init": fn(*mut Object);
        times_drop = "Discord_ActivityTimestamps_Drop": fn(*mut Object);
        times_set_start = "Discord_ActivityTimestamps_SetStart": fn(*mut Object, u64);
        times_set_end = "Discord_ActivityTimestamps_SetEnd": fn(*mut Object, u64);
        result_successful = "Discord_ClientResult_Successful": fn(*mut Object) -> bool;
        result_error = "Discord_ClientResult_Error": fn(*mut Object, *mut Text);
        result_drop = "Discord_ClientResult_Drop": fn(*mut Object);
    }

    fn api() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(Api::load).as_ref()
    }

    /// Discord's answer to a change: none yet, then taken or refused (with why).
    type Answer = Arc<Mutex<Option<Result<(), String>>>>;

    /// Called by Discord with its answer; the answer is ours to drop.
    unsafe extern "C" fn answered(result: *mut Object, answer: *mut c_void) {
        let Some(api) = api() else {
            return;
        };
        // SAFETY: `answer` is the Answer given with the change, kept until `release`.
        let answer = unsafe { &*answer.cast::<Answer>() };
        // SAFETY: a live result, dropped once.
        let outcome = unsafe {
            let outcome = if (api.result_successful)(result) {
                Ok(())
            } else {
                let mut text = Text {
                    ptr: null_mut(),
                    size: 0,
                };
                (api.result_error)(result, &mut text);
                let why = owned(api, text);
                Err(why)
            };
            (api.result_drop)(result);
            outcome
        };
        *answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
    }

    unsafe extern "C" fn release(answer: *mut c_void) {
        // SAFETY: made by Box::into_raw in `show`, released once by Discord.
        drop(unsafe { Box::from_raw(answer.cast::<Answer>()) });
    }

    unsafe extern "C" fn logged(message: Text, _severity: c_int, _: *mut c_void) {
        if let Some(api) = api() {
            // SAFETY: Discord hands over the message, for us to free.
            let message = unsafe { owned(api, message) };
            crate::log(false, &format!("Discord: {message}"));
        }
    }

    /// A string Discord made for us, copied, then freed.
    unsafe fn owned(api: &Api, text: Text) -> String {
        if text.ptr.is_null() {
            return String::new();
        }
        // SAFETY: Discord's string of `size` bytes.
        let s = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(text.ptr, text.size) })
            .into_owned();
        unsafe { (api.free)(text.ptr.cast()) };
        s
    }

    pub struct Sdk {
        api: &'static Api,
        client: Box<Object>,
        /// The last refusal, so the log has each one once, not every few seconds.
        refused: Option<String>,
    }

    impl Link for Sdk {
        fn connect(client_id: &str) -> Option<Self> {
            let api = api()?;
            let id = client_id.parse().ok()?;
            let mut client = Box::new(Object::new());
            // SAFETY: a new client, used on this thread only.
            unsafe {
                (api.client_init)(&mut *client);
                // 3 is Warning.
                (api.add_log_callback)(&mut *client, logged, None, null_mut(), 3);
                (api.set_application_id)(&mut *client, id);
            }
            Some(Self {
                api,
                client,
                refused: None,
            })
        }

        fn show(&mut self, activity: Option<&Activity>) -> std::io::Result<bool> {
            let api = self.api;
            let Some(activity) = activity else {
                // SAFETY: the live client.
                unsafe { (api.clear_rich_presence)(&mut *self.client) };
                return Ok(true);
            };
            let card = activity.card();
            let picture = |name: String| {
                if name == "needle" {
                    LOGO.to_string()
                } else {
                    name
                }
            };
            let answer: Answer = Answer::default();
            // SAFETY: objects made here and dropped here; Discord copies what it is given.
            unsafe {
                let mut a = Object::new();
                (api.activity_init)(&mut a);
                (api.activity_set_name)(&mut a, Text::of(&card.name));
                // Listening, with the name ("Listening to …") as the status.
                (api.activity_set_type)(&mut a, 2);
                (api.activity_set_display)(&mut a, &0);
                if let Some(details) = &card.details {
                    (api.activity_set_details)(&mut a, &Text::of(details));
                }
                if let Some(state) = &card.state {
                    (api.activity_set_state)(&mut a, &Text::of(state));
                }
                if let Some(large) = card.large_image.map(picture) {
                    let mut assets = Object::new();
                    (api.assets_init)(&mut assets);
                    (api.assets_set_large_image)(&mut assets, &Text::of(&large));
                    if let Some(text) = &card.large_text {
                        (api.assets_set_large_text)(&mut assets, &Text::of(text));
                    }
                    if let Some(small) = card.small_image.map(picture) {
                        (api.assets_set_small_image)(&mut assets, &Text::of(&small));
                    }
                    if let Some(text) = &card.small_text {
                        (api.assets_set_small_text)(&mut assets, &Text::of(text));
                    }
                    (api.activity_set_assets)(&mut a, &mut assets);
                    (api.assets_drop)(&mut assets);
                }
                if let Some(start) = card.start {
                    let mut times = Object::new();
                    (api.times_init)(&mut times);
                    (api.times_set_start)(&mut times, start as u64 * 1000);
                    if let Some(end) = card.end {
                        (api.times_set_end)(&mut times, end as u64 * 1000);
                    }
                    (api.activity_set_timestamps)(&mut a, &mut times);
                    (api.times_drop)(&mut times);
                }
                let user = Box::into_raw(Box::new(answer.clone()));
                (api.update_rich_presence)(
                    &mut *self.client,
                    &mut a,
                    answered,
                    Some(release),
                    user.cast(),
                );
                (api.activity_drop)(&mut a);
            }
            // Discord answers through its callbacks, which run here when asked.
            let until = Instant::now() + Duration::from_secs(10);
            loop {
                // SAFETY: runs Discord's waiting callbacks on this thread.
                unsafe { (api.run_callbacks)() };
                if let Some(outcome) = answer.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    return Ok(match outcome {
                        Ok(()) => {
                            self.refused = None;
                            true
                        }
                        Err(why) => {
                            if self.refused.as_ref() != Some(&why) {
                                needle_core::logfile::warn(format!(
                                    "Discord did not take the status: {why}"
                                ));
                                self.refused = Some(why);
                            }
                            false
                        }
                    });
                }
                if Instant::now() > until {
                    return Err(std::io::Error::other("Discord did not answer"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    impl Drop for Sdk {
        fn drop(&mut self) {
            // SAFETY: the live client, dropped once.
            unsafe { (self.api.client_drop)(&mut *self.client) };
        }
    }
}
