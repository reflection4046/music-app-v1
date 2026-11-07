//! Native Binary Entry Point
//! Runs the mobile music player interface in a desktop window configured
//! to mobile smartphone dimensions for rapid local development and testing.

fn main() -> eframe::Result<()> {
    rust_mobile_music_player::run_desktop()
}
