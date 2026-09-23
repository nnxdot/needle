#![recursion_limit = "1024"]
#![cfg_attr(windows, windows_subsystem = "windows")]
mod ui;
use clap::Parser;
#[derive(Parser)]
#[command(name = "Needle", version, about = "Your music, in its place")]
struct Options {
    #[arg(long)]
    data_dir: Option<std::path::PathBuf>,
    /// Music files to play (as File Explorer passes them).
    files: Vec<std::path::PathBuf>,
}
fn main() {
    let options = Options::parse();
    let directory = options
        .data_dir
        .unwrap_or_else(needle_core::database::Library::default_directory);
    // Needle is already open: give it the files, and let it come to the front.
    if needle_core::instance::hand_over(&directory, &options.files) {
        return;
    }
    needle_core::logfile::init(&directory);
    let files = options.files;
    let result =
        needle_core::database::Library::open(directory).and_then(|library| ui::run(library, files));
    if let Err(error) = result {
        needle_core::logfile::error(format!("Needle could not start: {error:#}"));
        rfd::MessageDialog::new()
            .set_title("Needle could not start")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
fn uuid_string() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        std::process::id()
    )
}
