//! Mobile & Desktop Rust Music Player
//! Crate root: exports 'android_main' entry point for Android Activity lifecycle
//! and provides the common application bootstrap for mobile and desktop.

pub mod audio;
pub mod playlist;
pub mod ui;

use audio::{spawn_audio_thread, AudioCommand, AudioEvent};
use ui::MusicPlayerApp;

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

/// Android Entry Point invoked by the Android NDK NativeActivity runtime.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("RustMusicPlayer"),
    );
    log::info!("Starting Rust Mobile Music Player on Android");

    let (cmd_sender, cmd_receiver) = crossbeam_channel::unbounded::<AudioCommand>();
    let (event_sender, event_receiver) = crossbeam_channel::unbounded::<AudioEvent>();

    // Spawn dedicated background audio playback thread
    spawn_audio_thread(cmd_receiver, event_sender);

    let mut options = eframe::NativeOptions::default();
    options.android_app = Some(app);

    let _ = eframe::run_native(
        "Rust Mobile Music Player",
        options,
        Box::new(|cc| {
            ui::configure_mobile_touch_styles(&cc.egui_ctx);
            Ok(Box::new(MusicPlayerApp::new(cc, cmd_sender, event_receiver)))
        }),
    );
}

/// Standalone desktop or mobile simulator runner
pub fn run_desktop() -> eframe::Result<()> {
    let (cmd_sender, cmd_receiver) = crossbeam_channel::unbounded::<AudioCommand>();
    let (event_sender, event_receiver) = crossbeam_channel::unbounded::<AudioEvent>();

    spawn_audio_thread(cmd_receiver, event_sender);

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Rust Mobile Music Player")
            .with_inner_size([400.0, 780.0]) // Smartphone aspect ratio
            .with_min_inner_size([360.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Mobile Music Player",
        native_options,
        Box::new(|cc| {
            ui::configure_mobile_touch_styles(&cc.egui_ctx);
            Ok(Box::new(MusicPlayerApp::new(cc, cmd_sender, event_receiver)))
        }),
    )
}
