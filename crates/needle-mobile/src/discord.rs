//! Discord's "Listening to" status from the phone, through Discord's own library (its Social
//! SDK). On Android the library hands the status to the Discord app on the phone; there is no
//! sign-in. An iPhone app cannot talk to another app, so there the person links their Discord
//! account once (Settings › Discord): Needle keeps the key Discord gives it in the Keychain
//! and talks to Discord itself. The library is not part of Needle's source, so the app has it
//! only when the build does (see MOBILE.md); the app then calls [`Needle::start_discord`].
use crate::{Needle, Result};

/// The Discord application Needle appears as, as on desktop.
#[cfg(any(target_os = "android", target_os = "ios"))]
const APP_ID: &str = "1552097597454295110";

/// Discord on this phone: whether the build has Discord's library, and on iPhone, the link to
/// the person's Discord account.
#[derive(Clone, uniffi::Record)]
pub struct DiscordAccount {
    /// This build has Discord's library.
    pub available: bool,
    /// The Discord account must be linked before the status can show (iPhone).
    pub needs_link: bool,
    pub linked: bool,
    /// Discord's sign-in page is open.
    pub linking: bool,
    /// Why linking, or the last connection, failed.
    pub problem: Option<String>,
}

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

    pub fn discord_account(&self) -> DiscordAccount {
        account_now()
    }

    /// iPhone: opens Discord's sign-in page, to link the person's Discord account. See
    /// [`Needle::discord_account`] for how it went.
    pub fn discord_link(&self) {
        #[cfg(target_os = "ios")]
        sdk::link(APP_ID);
    }

    /// iPhone: forgets the Discord account; the status goes away.
    pub fn discord_unlink(&self) {
        #[cfg(target_os = "ios")]
        sdk::account::unlink();
    }

    /// Keeps the Discord status in step with playback, from now on (when it is on in
    /// Settings). Call once, after the app has loaded Discord's library.
    pub fn start_discord(&self) {
        #[cfg(any(target_os = "android", target_os = "ios"))]
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

#[cfg(target_os = "android")]
fn account_now() -> DiscordAccount {
    DiscordAccount {
        available: sdk::available(),
        needs_link: false,
        linked: true,
        linking: false,
        problem: None,
    }
}

#[cfg(target_os = "ios")]
fn account_now() -> DiscordAccount {
    let (linking, problem) = sdk::account::state();
    DiscordAccount {
        available: sdk::available(),
        needs_link: true,
        linked: sdk::account::token().is_some(),
        linking,
        problem,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn account_now() -> DiscordAccount {
    DiscordAccount {
        available: false,
        needs_link: false,
        linked: false,
        linking: false,
        problem: None,
    }
}

/// As on desktop: tells Discord when the song, pause, or position changes, and clears the
/// status after a while of nothing playing.
#[cfg(any(target_os = "android", target_os = "ios"))]
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

/// Discord's library, called through its C interface. On Android the app loads it (Java needs
/// it first); on iPhone it is a framework inside the app. Here it is only looked up.
#[cfg(any(target_os = "android", target_os = "ios"))]
mod sdk {
    use needle_core::discord::{Activity, Link};
    use std::{
        ffi::{CStr, c_char, c_int, c_void},
        ptr::null_mut,
        sync::{Arc, Mutex, OnceLock},
        time::{Duration, Instant},
    };

    #[cfg(target_os = "android")]
    const LIBRARY: &CStr = c"libdiscord_partner_sdk.so";
    #[cfg(target_os = "ios")]
    const LIBRARY: &CStr = c"@executable_path/Frameworks/discord_partner_sdk.framework/discord_partner_sdk";

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

        /// For Discord to fill in.
        #[cfg(target_os = "ios")]
        fn empty() -> Self {
            Self {
                ptr: null_mut(),
                size: 0,
            }
        }
    }

    type Free = unsafe extern "C" fn(*mut c_void);
    type Answered = unsafe extern "C" fn(*mut Object, *mut c_void);
    type Logged = unsafe extern "C" fn(Text, c_int, *mut c_void);
    /// A sign-in's answer: the result, the code, and the address Discord sent it to.
    type Authorized = unsafe extern "C" fn(*mut Object, Text, Text, *mut c_void);
    /// A key's answer: the result, the access key, the refresh key, the key's type, its
    /// lifetime in seconds, and the scopes.
    type Exchanged = unsafe extern "C" fn(*mut Object, Text, Text, c_int, i32, Text, *mut c_void);

    unsafe extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    macro_rules! api {
        ($($field:ident = $name:literal: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
            // Android does not sign in, so some are only called on iPhone.
            #[allow(dead_code)]
            struct Api {
                $($field: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
            }

            impl Api {
                fn load() -> Option<Self> {
                    // SAFETY: looks up Discord's library, and functions with the types
                    // Discord's header (cdiscord.h) gives them.
                    unsafe {
                        let library = dlopen(LIBRARY.as_ptr(), RTLD_NOW);
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
        // Signing in (iPhone).
        client_connect = "Discord_Client_Connect": fn(*mut Object);
        client_status = "Discord_Client_GetStatus": fn(*mut Object) -> c_int;
        update_token = "Discord_Client_UpdateToken": fn(*mut Object, c_int, Text, Answered, Option<Free>, *mut c_void);
        refresh_token = "Discord_Client_RefreshToken": fn(*mut Object, u64, Text, Exchanged, Option<Free>, *mut c_void);
        get_token = "Discord_Client_GetToken": fn(*mut Object, u64, Text, Text, Text, Exchanged, Option<Free>, *mut c_void);
        authorize = "Discord_Client_Authorize": fn(*mut Object, *mut Object, Authorized, Option<Free>, *mut c_void);
        create_verifier = "Discord_Client_CreateAuthorizationCodeVerifier": fn(*mut Object, *mut Object);
        presence_scopes = "Discord_Client_GetDefaultPresenceScopes": fn(*mut Text);
        verifier_drop = "Discord_AuthorizationCodeVerifier_Drop": fn(*mut Object);
        verifier_challenge = "Discord_AuthorizationCodeVerifier_Challenge": fn(*mut Object, *mut Object);
        verifier_text = "Discord_AuthorizationCodeVerifier_Verifier": fn(*mut Object, *mut Text);
        challenge_drop = "Discord_AuthorizationCodeChallenge_Drop": fn(*mut Object);
        args_init = "Discord_AuthorizationArgs_Init": fn(*mut Object);
        args_drop = "Discord_AuthorizationArgs_Drop": fn(*mut Object);
        args_set_client_id = "Discord_AuthorizationArgs_SetClientId": fn(*mut Object, u64);
        args_set_scopes = "Discord_AuthorizationArgs_SetScopes": fn(*mut Object, Text);
        args_set_challenge = "Discord_AuthorizationArgs_SetCodeChallenge": fn(*mut Object, *mut Object);
    }

    fn api() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(Api::load).as_ref()
    }

    /// This build has Discord's library.
    pub fn available() -> bool {
        api().is_some()
    }

    /// Where an answer from Discord lands: none yet, then the answer.
    type Slot<T> = Arc<Mutex<Option<T>>>;
    /// Discord took a change, or refused it (with why).
    type Outcome = Result<(), String>;

    /// A slot to hand Discord with a request; Discord releases it with [`drop_slot`].
    fn give<T>(slot: &Slot<T>) -> *mut c_void {
        Box::into_raw(Box::new(slot.clone())).cast()
    }

    unsafe extern "C" fn drop_slot<T>(slot: *mut c_void) {
        // SAFETY: made by `give`, released once by Discord.
        drop(unsafe { Box::from_raw(slot.cast::<Slot<T>>()) });
    }

    /// Puts Discord's answer in the slot given with the request.
    unsafe fn put<T>(slot: *mut c_void, value: T) {
        // SAFETY: a slot made by `give`, kept until Discord releases it.
        let slot = unsafe { &*slot.cast::<Slot<T>>() };
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }

    /// Discord answers through its callbacks, which run on this thread when asked: runs them
    /// until the answer comes, or `limit` passes.
    fn wait_for<T>(api: &Api, slot: &Slot<T>, limit: Duration) -> Option<T> {
        let until = Instant::now() + limit;
        loop {
            // SAFETY: runs Discord's waiting callbacks on this thread.
            unsafe { (api.run_callbacks)() };
            if let Some(answer) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return Some(answer);
            }
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Whether a request worked, and drops its result.
    unsafe fn outcome(api: &Api, result: *mut Object) -> Outcome {
        // SAFETY: a live result, dropped once.
        unsafe {
            let outcome = if (api.result_successful)(result) {
                Ok(())
            } else {
                let mut text = Text {
                    ptr: null_mut(),
                    size: 0,
                };
                (api.result_error)(result, &mut text);
                Err(owned(api, text))
            };
            (api.result_drop)(result);
            outcome
        }
    }

    unsafe extern "C" fn answered(result: *mut Object, slot: *mut c_void) {
        if let Some(api) = api() {
            // SAFETY: Discord's result, and the slot given with the request.
            unsafe { put(slot, outcome(api, result)) };
        }
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
        /// iPhone: the unlinks before this connection (see [`account::UNLINKED`]).
        #[cfg(target_os = "ios")]
        unlinked: u64,
        /// iPhone: when the access key runs out, a while early.
        #[cfg(target_os = "ios")]
        expires: Instant,
    }

    impl Sdk {
        /// A new client of Discord's, as Needle's application.
        fn new(api: &'static Api, id: u64) -> Self {
            let mut client = Box::new(Object::new());
            // SAFETY: a new client, used on this thread only.
            unsafe {
                (api.client_init)(&mut *client);
                // 3 is Warning.
                (api.add_log_callback)(&mut *client, logged, None, null_mut(), 3);
                (api.set_application_id)(&mut *client, id);
            }
            Self {
                api,
                client,
                refused: None,
                #[cfg(target_os = "ios")]
                unlinked: account::UNLINKED.load(std::sync::atomic::Ordering::SeqCst),
                #[cfg(target_os = "ios")]
                expires: Instant::now(),
            }
        }
    }

    impl Link for Sdk {
        fn connect(client_id: &str) -> Option<Self> {
            let api = api()?;
            let id = client_id.parse().ok()?;
            // Without a linked account there is nothing to connect to.
            #[cfg(target_os = "ios")]
            let refresh = account::token()?;
            #[allow(unused_mut)]
            let mut sdk = Self::new(api, id);
            #[cfg(target_os = "ios")]
            if !sdk.sign_in(id, &refresh) {
                return None;
            }
            Some(sdk)
        }

        fn show(&mut self, activity: Option<&Activity>) -> std::io::Result<bool> {
            let api = self.api;
            // Unlinked, or the key is running out: let go, and connect again (or not).
            #[cfg(target_os = "ios")]
            if account::UNLINKED.load(std::sync::atomic::Ordering::SeqCst) != self.unlinked
                || Instant::now() > self.expires
            {
                return Err(std::io::Error::other("Connecting to Discord again"));
            }
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
            let answer: Slot<Outcome> = Slot::default();
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
                (api.update_rich_presence)(
                    &mut *self.client,
                    &mut a,
                    answered,
                    Some(drop_slot::<Outcome>),
                    give(&answer),
                );
                (api.activity_drop)(&mut a);
            }
            match wait_for(api, &answer, Duration::from_secs(10)) {
                Some(Ok(())) => {
                    self.refused = None;
                    Ok(true)
                }
                Some(Err(why)) => {
                    if self.refused.as_ref() != Some(&why) {
                        needle_core::logfile::warn(format!("Discord did not take the status: {why}"));
                        self.refused = Some(why);
                    }
                    Ok(false)
                }
                None => Err(std::io::Error::other("Discord did not answer")),
            }
        }
    }

    impl Drop for Sdk {
        fn drop(&mut self) {
            // SAFETY: the live client, dropped once.
            unsafe { (self.api.client_drop)(&mut *self.client) };
        }
    }

    /// A key from Discord: the access key, the refresh key, and the access key's lifetime in
    /// seconds.
    #[cfg(target_os = "ios")]
    type Tokens = Result<(String, String, i32), String>;

    #[cfg(target_os = "ios")]
    unsafe extern "C" fn exchanged(
        result: *mut Object,
        access: Text,
        refresh: Text,
        _kind: c_int,
        lifetime: i32,
        scopes: Text,
        slot: *mut c_void,
    ) {
        let Some(api) = api() else {
            return;
        };
        // SAFETY: Discord hands over the strings, for us to free, and its result.
        unsafe {
            let (access, refresh) = (owned(api, access), owned(api, refresh));
            owned(api, scopes);
            let tokens = outcome(api, result).map(|()| (access, refresh, lifetime));
            put::<Tokens>(slot, tokens);
        }
    }

    /// The code Discord's sign-in gives, and the address it was sent to.
    #[cfg(target_os = "ios")]
    type Code = Result<(String, String), String>;

    #[cfg(target_os = "ios")]
    unsafe extern "C" fn authorized(result: *mut Object, code: Text, redirect: Text, slot: *mut c_void) {
        let Some(api) = api() else {
            return;
        };
        // SAFETY: Discord hands over the strings, for us to free, and its result.
        unsafe {
            let (code, redirect) = (owned(api, code), owned(api, redirect));
            let answer = outcome(api, result).map(|()| (code, redirect));
            put::<Code>(slot, answer);
        }
    }

    #[cfg(target_os = "ios")]
    impl Sdk {
        /// Signs in with the linked account's refresh key (Discord gives a new one each
        /// time), then connects. False when that did not work.
        fn sign_in(&mut self, id: u64, refresh: &str) -> bool {
            let api = self.api;
            let tokens: Slot<Tokens> = Slot::default();
            // SAFETY: the live client; Discord copies the key.
            unsafe {
                (api.refresh_token)(
                    &mut *self.client,
                    id,
                    Text::of(refresh),
                    exchanged,
                    Some(drop_slot::<Tokens>),
                    give(&tokens),
                )
            };
            let (access, refresh, lifetime) = match wait_for(api, &tokens, Duration::from_secs(30)) {
                Some(Ok(tokens)) => tokens,
                Some(Err(why)) => {
                    account::problem(Some(format!(
                        "Discord did not take Needle's key ({why}). Link your account again."
                    )));
                    return false;
                }
                // No answer (no internet): try again later.
                None => return false,
            };
            account::save(&refresh);
            // An hour early, so it never runs out while connected.
            self.expires = Instant::now()
                + Duration::from_secs((lifetime.max(0) as u64).saturating_sub(3600).max(600));
            let done: Slot<Outcome> = Slot::default();
            // SAFETY: the live client; Discord copies the key. 1 is a Bearer key.
            unsafe {
                (api.update_token)(
                    &mut *self.client,
                    1,
                    Text::of(&access),
                    answered,
                    Some(drop_slot::<Outcome>),
                    give(&done),
                )
            };
            match wait_for(api, &done, Duration::from_secs(30)) {
                Some(Ok(())) => {}
                Some(Err(why)) => {
                    account::problem(Some(format!("Discord did not take Needle's key ({why})")));
                    return false;
                }
                None => return false,
            }
            // SAFETY: the live client.
            unsafe { (api.client_connect)(&mut *self.client) };
            let until = Instant::now() + Duration::from_secs(30);
            loop {
                // SAFETY: runs Discord's callbacks, and reads the live client's status.
                let status = unsafe {
                    (api.run_callbacks)();
                    (api.client_status)(&mut *self.client)
                };
                // 3 is Ready.
                if status == 3 {
                    account::problem(None);
                    return true;
                }
                if Instant::now() > until {
                    needle_core::logfile::warn("Discord did not connect");
                    return false;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    /// iPhone: opens Discord's sign-in page, and keeps the key Discord gives when the person
    /// allows Needle. In the background; [`account::state`] tells how it goes.
    #[cfg(target_os = "ios")]
    pub fn link(client_id: &'static str) {
        use std::sync::atomic::Ordering;
        if account::LINKING.swap(true, Ordering::SeqCst) {
            return;
        }
        account::problem(None);
        std::thread::spawn(move || {
            if let Err(why) = linked(client_id) {
                needle_core::logfile::warn(format!("Linking Discord: {why}"));
                account::problem(Some(why));
            }
            account::LINKING.store(false, Ordering::SeqCst);
        });
    }

    #[cfg(target_os = "ios")]
    fn linked(client_id: &str) -> Result<(), String> {
        let api = api().ok_or("This build of Needle has no Discord library")?;
        let id: u64 = client_id.parse().map_err(|_| "Needle's Discord application is wrong")?;
        let mut sdk = Sdk::new(api, id);
        let code: Slot<Code> = Slot::default();
        // SAFETY: objects made here and dropped here; Discord copies what it is given.
        let verifier = unsafe {
            let mut verifier = Object::new();
            (api.create_verifier)(&mut *sdk.client, &mut verifier);
            let mut challenge = Object::new();
            (api.verifier_challenge)(&mut verifier, &mut challenge);
            let mut text = Text::empty();
            (api.verifier_text)(&mut verifier, &mut text);
            let verifier_text = owned(api, text);
            let mut scopes = Text::empty();
            (api.presence_scopes)(&mut scopes);
            let scopes = owned(api, scopes);
            let mut args = Object::new();
            (api.args_init)(&mut args);
            (api.args_set_client_id)(&mut args, id);
            (api.args_set_scopes)(&mut args, Text::of(&scopes));
            (api.args_set_challenge)(&mut args, &mut challenge);
            (api.authorize)(
                &mut *sdk.client,
                &mut args,
                authorized,
                Some(drop_slot::<Code>),
                give(&code),
            );
            (api.args_drop)(&mut args);
            (api.challenge_drop)(&mut challenge);
            (api.verifier_drop)(&mut verifier);
            verifier_text
        };
        // The person signs in on Discord's page: give them time.
        let (code, redirect) = wait_for(api, &code, Duration::from_secs(600))
            .ok_or("Discord's sign-in page was open too long")??;
        let tokens: Slot<Tokens> = Slot::default();
        // SAFETY: the live client; Discord copies what it is given.
        unsafe {
            (api.get_token)(
                &mut *sdk.client,
                id,
                Text::of(&code),
                Text::of(&verifier),
                Text::of(&redirect),
                exchanged,
                Some(drop_slot::<Tokens>),
                give(&tokens),
            )
        };
        let (_, refresh, _) =
            wait_for(api, &tokens, Duration::from_secs(30)).ok_or("Discord did not answer")??;
        account::save(&refresh);
        if account::token().as_deref() != Some(refresh.as_str()) {
            return Err("Needle could not keep Discord's key in the Keychain".into());
        }
        drop(sdk);
        Ok(())
    }

    /// iPhone: the linked Discord account. Its refresh key is kept in the Keychain.
    #[cfg(target_os = "ios")]
    pub mod account {
        use needle_core::secret_store as plugin;
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        };

        const PLACE: &str = "discord";
        const KEY: &str = "refresh_token";
        pub static LINKING: AtomicBool = AtomicBool::new(false);
        /// Goes up when the account is unlinked, so a connection made before lets go.
        pub static UNLINKED: AtomicU64 = AtomicU64::new(0);
        static PROBLEM: Mutex<Option<String>> = Mutex::new(None);

        pub fn token() -> Option<String> {
            plugin::get(PLACE, KEY).ok().flatten()
        }

        pub fn save(token: &str) {
            if let Err(e) = plugin::set(PLACE, KEY, token) {
                needle_core::logfile::warn(format!("Keeping Discord's key: {e:#}"));
            }
        }

        pub fn unlink() {
            let _ = plugin::delete(PLACE, KEY);
            UNLINKED.fetch_add(1, Ordering::SeqCst);
            problem(None);
        }

        /// Sets why linking or connecting failed (logged once), or clears it.
        pub fn problem(why: Option<String>) {
            let mut now = PROBLEM.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(why) = &why
                && now.as_ref() != Some(why)
            {
                needle_core::logfile::warn(format!("Discord: {why}"));
            }
            *now = why;
        }

        /// Whether Discord's sign-in page is open, and why linking or connecting failed.
        pub fn state() -> (bool, Option<String>) {
            (
                LINKING.load(Ordering::SeqCst),
                PROBLEM.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            )
        }
    }
}
