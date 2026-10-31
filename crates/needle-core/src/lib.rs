pub mod analysis;
pub mod artists;
pub mod audio;
pub mod audio_file;
pub mod browse;
pub mod cast;
pub mod crossfade;
pub mod cue;
pub mod database;
pub mod demo;
pub mod discord;
pub mod doctor;
pub mod dsp;
pub mod effects;
#[cfg(windows)]
mod exclusive;
pub mod ffmpeg;
pub mod find;
pub mod formats;
pub mod grouping;
pub mod history;
pub mod import;
pub mod instance;
pub mod integrations;
pub mod logfile;
pub mod mcp;
pub mod media;
#[cfg(windows)]
pub mod mediafoundation;
pub mod model;
mod mp4_trim;
pub mod numbers;
mod opus;
pub mod platform_output;
pub mod plugins;
pub mod query;
pub mod radio;
pub mod remote;
pub mod rules;
pub mod scan;
pub mod scope;
mod secrets;
/// Secrets kept in the system store (the Keychain on iPhone), by owner and key.
pub use secrets::plugin as secret_store;
/// Android: where the app keeps its secrets (see `secrets`).
#[cfg(target_os = "android")]
pub use secrets::set_android_folder;
/// The library folder: a test copy's, with a `secrets.json` in it, keeps its secrets there.
pub use secrets::set_secrets_folder;
pub mod sources;
pub mod stems;
pub mod subsonic;
pub mod sync;
pub mod tempo;
pub mod timing;
pub mod update;
pub mod waveform;
pub mod wrapped;
