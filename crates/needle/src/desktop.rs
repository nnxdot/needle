#![recursion_limit = "1024"]
#![cfg_attr(windows, windows_subsystem = "windows")]
mod ui;
use clap::Parser;
#[derive(Parser)]
#[command(name = "Needle", version, about = "Your music, in its place")]
struct Options {
    #[arg(long)]
    data_dir: Option<std::path::PathBuf>,
}
fn main() {
    let options = Options::parse();
    let result = needle_core::database::Library::open(
        options
            .data_dir
            .unwrap_or_else(needle_core::database::Library::default_directory),
    )
    .and_then(ui::run);
    if let Err(error) = result {
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
