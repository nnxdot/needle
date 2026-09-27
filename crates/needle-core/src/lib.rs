pub mod analysis;
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
pub mod formats;
pub mod history;
pub mod import;
pub mod instance;
pub mod integrations;
pub mod logfile;
pub mod media;
#[cfg(windows)]
pub mod mediafoundation;
pub mod model;
mod mp4_trim;
mod opus;
pub mod plugins;
pub mod query;
pub mod radio;
pub mod remote;
pub mod rules;
pub mod scan;
mod secrets;
/// Android: where the app keeps its secrets (see `secrets`).
#[cfg(target_os = "android")]
pub use secrets::set_android_folder;
pub mod sources;
pub mod stems;
pub mod sync;
pub mod update;
pub mod wrapped;
