//! Touch-First Mobile Immediate-Mode UI with egui / eframe
//! Optimized for smartphone screens: thumb-friendly touch targets (>=44px),
//! floating mini-player bar, expandable full-screen Now Playing drawer.

use crate::audio::{AudioCommand, AudioEvent};
use crate::playlist::{format_duration, scan_directory_for_audio, Playlist, RepeatMode, Track};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, Margin, Pos2, Sense, Vec2};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayStatus {
    Stopped,
    Playing,
    Paused,
}

pub struct MusicPlayerApp {
    cmd_sender: Sender<AudioCommand>,
    event_receiver: Receiver<AudioEvent>,

    playlist: Playlist,
    status: PlayStatus,
    current_position: Duration,
    total_duration: Duration,

    volume: f32,
    is_muted: bool,

    is_seeking: bool,
    seek_drag_pos: f32,

    show_now_playing_sheet: bool,
}

impl MusicPlayerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        cmd_sender: Sender<AudioCommand>,
        event_receiver: Receiver<AudioEvent>,
    ) -> Self {
        Self {
            cmd_sender,
            event_receiver,
            playlist: Playlist::new(),
            status: PlayStatus::Stopped,
            current_position: Duration::ZERO,
            total_duration: Duration::ZERO,
            volume: 0.85,
            is_muted: false,
            is_seeking: false,
            seek_drag_pos: 0.0,
            show_now_playing_sheet: false,
        }
    }

    fn handle_audio_events(&mut self) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AudioEvent::PlaybackStarted { total_duration, .. } => {
                    self.status = PlayStatus::Playing;
                    if let Some(dur) = total_duration {
                        self.total_duration = dur;
                    }
                }
                AudioEvent::Progress { position, total_duration } => {
                    if !self.is_seeking {
                        self.current_position = position;
                    }
                    if let Some(dur) = total_duration {
                        self.total_duration = dur;
                    }
                }
                AudioEvent::PlaybackFinished => {
                    self.status = PlayStatus::Stopped;
                    self.current_position = Duration::ZERO;
                    self.next_track(true);
                }
                AudioEvent::Error(_err) => {
                    self.status = PlayStatus::Stopped;
                }
            }
        }
    }

    fn play_index(&mut self, index: usize) {
        if let Some(track) = self.playlist.tracks.get(index) {
            let path = track.path.clone();
            self.playlist.current_index = Some(index);
            self.total_duration = track.duration;
            self.current_position = Duration::ZERO;
            let _ = self.cmd_sender.send(AudioCommand::Play(path));
            self.status = PlayStatus::Playing;
        }
    }

    fn toggle_play_pause(&mut self) {
        match self.status {
            PlayStatus::Playing => {
                let _ = self.cmd_sender.send(AudioCommand::Pause);
                self.status = PlayStatus::Paused;
            }
            PlayStatus::Paused => {
                let _ = self.cmd_sender.send(AudioCommand::Resume);
                self.status = PlayStatus::Playing;
            }
            PlayStatus::Stopped => {
                if let Some(idx) = self.playlist.current_index {
                    self.play_index(idx);
                } else if !self.playlist.tracks.is_empty() {
                    self.play_index(0);
                }
            }
        }
    }

    fn next_track(&mut self, natural_finish: bool) {
        if let Some(next_idx) = self.playlist.get_next_index(natural_finish) {
            self.play_index(next_idx);
        } else {
            self.status = PlayStatus::Stopped;
            let _ = self.cmd_sender.send(AudioCommand::Stop);
        }
    }

    fn previous_track(&mut self) {
        if self.current_position.as_secs() > 3 {
            let _ = self.cmd_sender.send(AudioCommand::Seek(Duration::ZERO));
            self.current_position = Duration::ZERO;
            return;
        }
        if let Some(prev_idx) = self.playlist.get_previous_index() {
            self.play_index(prev_idx);
        }
    }

    fn pick_audio_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title("Select Audio Files")
            .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a"])
            .pick_files()
        {
            for path in paths {
                if let Ok(track) = Track::from_path(path) {
                    self.playlist.add_track(track);
                }
            }
        }
    }

    fn pick_audio_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Select Music Folder")
            .pick_folder()
        {
            let tracks = scan_directory_for_audio(dir);
            self.playlist.add_tracks(tracks);
        }
    }
}

impl eframe::App for MusicPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.status == PlayStatus::Playing {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        self.handle_audio_events();

        // Mobile Header Bar (52px)
        egui::TopBottomPanel::top("mobile_header")
            .frame(egui::Frame::none().fill(Color32::from_rgb(15, 17, 23)).inner_margin(Margin::symmetric(16.0, 12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Music").strong().size(20.0).color(Color32::WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([44.0, 36.0], egui::Button::new("📂")).clicked() {
                            self.pick_audio_folder();
                        }
                        if ui.add_sized([44.0, 36.0], egui::Button::new("＋")).clicked() {
                            self.pick_audio_files();
                        }
                    });
                });
            });

        // Mobile Bottom Bar with Mini Player
        egui::TopBottomPanel::bottom("mobile_bottom_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgb(18, 20, 26)).inner_margin(Margin::same(8.0)))
            .show(ctx, |ui| {
                if let Some(track) = self.playlist.current_track() {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(28, 33, 44))
                        .rounding(12.0)
                        .inner_margin(Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let label_resp = ui.allocate_ui(Vec2::new(ui.available_width() - 90.0, 36.0), |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&track.title).strong().color(Color32::WHITE).truncate());
                                        ui.label(egui::RichText::new(&track.artist).size(11.0).color(Color32::from_rgb(148, 163, 184)).truncate());
                                    });
                                });

                                if label_resp.response.interact(Sense::click()).clicked() {
                                    self.show_now_playing_sheet = true;
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let play_label = if self.status == PlayStatus::Playing { "⏸" } else { "▶" };
                                    if ui.add_sized([44.0, 40.0], egui::Button::new(egui::RichText::new(play_label).strong())).clicked() {
                                        self.toggle_play_pause();
                                    }
                                    if ui.add_sized([36.0, 40.0], egui::Button::new("⏭")).clicked() {
                                        self.next_track(false);
                                    }
                                });
                            });
                        });
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    let tab_w = (ui.available_width() - 8.0) / 3.0;
                    let _ = ui.add_sized([tab_w, 44.0], egui::Button::new("🎵 Songs"));
                    if ui.add_sized([tab_w, 44.0], egui::Button::new("💽 Player")).clicked() {
                        self.show_now_playing_sheet = true;
                    }
                    if ui.add_sized([tab_w, 44.0], egui::Button::new("📁 Folders")).clicked() {
                        self.pick_audio_folder();
                    }
                });
            });

        // Center song list
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(11, 13, 17)).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                if self.playlist.tracks.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(100.0);
                        ui.label(egui::RichText::new("No Tracks Loaded").size(18.0).strong().color(Color32::WHITE));
                        ui.add_space(8.0);
                        ui.label("Tap '＋' to pick songs from your device.");
                    });
                    return;
                }

                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut track_to_play = None;

                    for (idx, track) in self.playlist.tracks.iter().enumerate() {
                        let is_active = self.playlist.current_index == Some(idx);
                        let bg = if is_active {
                            Color32::from_rgb(26, 35, 52)
                        } else {
                            Color32::from_rgb(18, 21, 28)
                        };

                        egui::Frame::none()
                            .fill(bg)
                            .rounding(8.0)
                            .inner_margin(Margin::symmetric(14.0, 10.0))
                            .show(ui, |ui| {
                                let row_resp = ui.horizontal(|ui| {
                                    let icon = if is_active && self.status == PlayStatus::Playing { "▶" } else { "♫" };
                                    let color = if is_active { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgb(100, 116, 139) };
                                    ui.label(egui::RichText::new(icon).color(color).size(15.0));
                                    ui.add_space(10.0);

                                    ui.vertical(|ui| {
                                        let text_color = if is_active { Color32::from_rgb(56, 189, 248) } else { Color32::WHITE };
                                        ui.label(egui::RichText::new(&track.title).strong().color(text_color).truncate());
                                        ui.label(egui::RichText::new(&track.artist).size(12.0).color(Color32::from_rgb(148, 163, 184)).truncate());
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.monospace(egui::RichText::new(track.formatted_duration()).size(12.0).color(Color32::from_rgb(148, 163, 184)));
                                    });
                                });

                                if row_resp.response.interact(Sense::click()).clicked() {
                                    track_to_play = Some(idx);
                                }
                            });

                        ui.add_space(4.0);
                    }

                    if let Some(idx) = track_to_play {
                        self.play_index(idx);
                        self.show_now_playing_sheet = true;
                    }
                });
            });

        // Slide-up drawer
        if self.show_now_playing_sheet {
            egui::Window::new("Now Playing")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .fixed_pos(Pos2::new(0.0, 0.0))
                .fixed_size(ctx.screen_rect().size())
                .frame(egui::Frame::none().fill(Color32::from_rgb(13, 15, 20)).inner_margin(Margin::same(20.0)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.add_sized([44.0, 44.0], egui::Button::new("⌄")).clicked() {
                            self.show_now_playing_sheet = false;
                        }
                        ui.label(egui::RichText::new("Now Playing").strong().size(16.0).color(Color32::from_rgb(203, 213, 225)));
                    });

                    ui.add_space(20.0);

                    let art_size = ui.available_width().min(300.0);
                    ui.vertical_centered(|ui| {
                        let (rect, _response) = ui.allocate_exact_size(Vec2::splat(art_size), Sense::hover());
                        ui.painter().rect_filled(rect, 16.0, Color32::from_rgb(28, 33, 44));
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "💽",
                            egui::FontId::proportional(64.0),
                            Color32::from_rgb(56, 189, 248),
                        );
                    });

                    ui.add_space(24.0);

                    if let Some(track) = self.playlist.current_track() {
                        ui.label(egui::RichText::new(&track.title).strong().size(22.0).color(Color32::WHITE));
                        ui.label(egui::RichText::new(&track.artist).size(15.0).color(Color32::from_rgb(148, 163, 184)));
                    }

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        let cur_str = format_duration(self.current_position);
                        ui.monospace(egui::RichText::new(cur_str).size(12.0).color(Color32::from_rgb(148, 163, 184)));

                        let total_secs = self.total_duration.as_secs_f32();
                        let mut current_ratio = if total_secs > 0.0 {
                            (self.current_position.as_secs_f32() / total_secs).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };

                        let slider_width = ui.available_width() - 50.0;
                        let slider = egui::Slider::new(&mut current_ratio, 0.0..=1.0)
                            .show_value(false)
                            .trailing_fill(true);

                        let resp = ui.add_sized([slider_width, 24.0], slider);
                        if resp.drag_stopped() {
                            let target_dur = Duration::from_secs_f32(current_ratio * total_secs);
                            self.current_position = target_dur;
                            let _ = self.cmd_sender.send(AudioCommand::Seek(target_dur));
                        }

                        ui.monospace(egui::RichText::new(format_duration(self.total_duration)).size(12.0).color(Color32::from_rgb(148, 163, 184)));
                    });

                    ui.add_space(24.0);

                    ui.horizontal(|ui| {
                        let btn_w = (ui.available_width() - 20.0) / 5.0;

                        let shuf_col = if self.playlist.is_shuffle { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgb(148, 163, 184) };
                        if ui.add_sized([btn_w, 48.0], egui::Button::new(egui::RichText::new("🔀").color(shuf_col))).clicked() {
                            self.playlist.toggle_shuffle();
                        }

                        if ui.add_sized([btn_w, 48.0], egui::Button::new("⏮")).clicked() {
                            self.previous_track();
                        }

                        let play_icon = if self.status == PlayStatus::Playing { "⏸" } else { "▶" };
                        if ui.add_sized([btn_w + 10.0, 56.0], egui::Button::new(egui::RichText::new(play_icon).size(20.0).strong())).clicked() {
                            self.toggle_play_pause();
                        }

                        if ui.add_sized([btn_w, 48.0], egui::Button::new("⏭")).clicked() {
                            self.next_track(false);
                        }

                        let rep_icon = match self.playlist.repeat_mode {
                            RepeatMode::Off => "🔁",
                            RepeatMode::RepeatOne => "🔂",
                            RepeatMode::RepeatAll => "🔁",
                        };
                        let rep_col = if self.playlist.repeat_mode != RepeatMode::Off { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgb(148, 163, 184) };
                        if ui.add_sized([btn_w, 48.0], egui::Button::new(egui::RichText::new(rep_icon).color(rep_col))).clicked() {
                            self.playlist.cycle_repeat();
                        }
                    });

                    ui.add_space(24.0);

                    ui.horizontal(|ui| {
                        let mute_icon = if self.is_muted { "🔇" } else { "🔊" };
                        if ui.add_sized([44.0, 44.0], egui::Button::new(mute_icon)).clicked() {
                            self.is_muted = !self.is_muted;
                            let _ = self.cmd_sender.send(AudioCommand::SetMute(self.is_muted));
                        }

                        let vol_width = ui.available_width() - 40.0;
                        let slider = egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false).trailing_fill(true);
                        let v_resp = ui.add_sized([vol_width, 24.0], slider);
                        if v_resp.changed() {
                            if self.is_muted && self.volume > 0.0 {
                                self.is_muted = false;
                            }
                            let _ = self.cmd_sender.send(AudioCommand::SetVolume(self.volume));
                        }
                    });
                });
        }
    }
}

pub fn configure_mobile_touch_styles(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(11, 13, 17);
    visuals.panel_fill = Color32::from_rgb(11, 13, 17);
    visuals.extreme_bg_color = Color32::from_rgb(8, 10, 14);
    ctx.set_visuals(visuals);
}
