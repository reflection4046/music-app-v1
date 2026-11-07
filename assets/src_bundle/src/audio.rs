//! Background Audio Engine Thread
//! Runs on a dedicated OS thread. Decodes audio files using rodio and symphonia,
//! manages playback state, volume, seeking, and emits periodic 50ms progress updates.

use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// Commands sent from the Mobile UI to the Audio Engine.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    SetMute(bool),
}

/// Events emitted from the Audio Engine back to the Mobile UI.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    PlaybackStarted {
        path: PathBuf,
        total_duration: Option<Duration>,
    },
    Progress {
        position: Duration,
        total_duration: Option<Duration>,
    },
    PlaybackFinished,
    Error(String),
}

struct AudioState {
    sink: Option<Sink>,
    current_path: Option<PathBuf>,
    is_paused: bool,
    volume: f32,
    is_muted: bool,
    total_duration: Option<Duration>,
    base_offset: Duration,
    resume_instant: Option<Instant>,
}

impl AudioState {
    fn new() -> Self {
        Self {
            sink: None,
            current_path: None,
            is_paused: false,
            volume: 0.85,
            is_muted: false,
            total_duration: None,
            base_offset: Duration::ZERO,
            resume_instant: None,
        }
    }

    fn current_position(&self) -> Duration {
        if self.sink.is_none() {
            return Duration::ZERO;
        }

        if self.is_paused {
            self.base_offset
        } else if let Some(instant) = self.resume_instant {
            let elapsed = instant.elapsed();
            let total = self.base_offset + elapsed;
            if let Some(dur) = self.total_duration {
                total.min(dur)
            } else {
                total
            }
        } else {
            self.base_offset
        }
    }

    fn apply_volume(&self) {
        if let Some(sink) = &self.sink {
            if self.is_muted {
                sink.set_volume(0.0);
            } else {
                sink.set_volume(self.volume);
            }
        }
    }
}

pub fn spawn_audio_thread(
    cmd_receiver: Receiver<AudioCommand>,
    event_sender: Sender<AudioEvent>,
) {
    thread::Builder::new()
        .name("mobile_audio_engine".into())
        .spawn(move || {
            let (_stream, stream_handle): (OutputStream, OutputStreamHandle) = match OutputStream::try_default() {
                Ok(res) => res,
                Err(err) => {
                    let _ = event_sender.send(AudioEvent::Error(format!(
                        "Audio output device initialization failed: {err}"
                    )));
                    return;
                }
            };

            let mut state = AudioState::new();
            let tick_rate = Duration::from_millis(50);
            let mut last_progress_send = Instant::now();

            loop {
                match cmd_receiver.recv_timeout(tick_rate) {
                    Ok(command) => match command {
                        AudioCommand::Play(path) => {
                            match play_file(&stream_handle, &mut state, path.clone()) {
                                Ok(duration) => {
                                    state.total_duration = duration;
                                    state.base_offset = Duration::ZERO;
                                    state.resume_instant = Some(Instant::now());
                                    state.is_paused = false;
                                    state.current_path = Some(path.clone());

                                    let _ = event_sender.send(AudioEvent::PlaybackStarted {
                                        path,
                                        total_duration: duration,
                                    });
                                }
                                Err(err) => {
                                    let _ = event_sender.send(AudioEvent::Error(err));
                                }
                            }
                        }
                        AudioCommand::Pause => {
                            if let Some(sink) = &state.sink {
                                if !state.is_paused {
                                    sink.pause();
                                    state.base_offset = state.current_position();
                                    state.resume_instant = None;
                                    state.is_paused = true;
                                }
                            }
                        }
                        AudioCommand::Resume => {
                            if let Some(sink) = &state.sink {
                                if state.is_paused {
                                    sink.play();
                                    state.resume_instant = Some(Instant::now());
                                    state.is_paused = false;
                                }
                            }
                        }
                        AudioCommand::Stop => {
                            state.sink = None;
                            state.current_path = None;
                            state.is_paused = false;
                            state.base_offset = Duration::ZERO;
                            state.resume_instant = None;
                            state.total_duration = None;
                        }
                        AudioCommand::Seek(target) => {
                            if let Some(sink) = &mut state.sink {
                                if sink.try_seek(target).is_ok() {
                                    state.base_offset = target;
                                    if !state.is_paused {
                                        state.resume_instant = Some(Instant::now());
                                    }
                                } else if let Some(path) = state.current_path.clone() {
                                    if let Ok(duration) = play_file(&stream_handle, &mut state, path) {
                                        state.total_duration = duration;
                                        if let Some(s) = &state.sink {
                                            let _ = s.try_seek(target);
                                        }
                                        state.base_offset = target;
                                        if state.is_paused {
                                            if let Some(s) = &state.sink {
                                                s.pause();
                                            }
                                        } else {
                                            state.resume_instant = Some(Instant::now());
                                        }
                                    }
                                }
                            }
                        }
                        AudioCommand::SetVolume(vol) => {
                            state.volume = vol.clamp(0.0, 1.0);
                            state.apply_volume();
                        }
                        AudioCommand::SetMute(muted) => {
                            state.is_muted = muted;
                            state.apply_volume();
                        }
                    },
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }

                if let Some(sink) = &state.sink {
                    if sink.empty() && !state.is_paused {
                        state.sink = None;
                        state.current_path = None;
                        state.resume_instant = None;
                        let _ = event_sender.send(AudioEvent::PlaybackFinished);
                        continue;
                    }

                    if last_progress_send.elapsed() >= Duration::from_millis(50) {
                        last_progress_send = Instant::now();
                        let pos = state.current_position();
                        let _ = event_sender.send(AudioEvent::Progress {
                            position: pos,
                            total_duration: state.total_duration,
                        });
                    }
                }
            }
        })
        .expect("Failed to spawn mobile audio worker thread");
}

fn play_file(
    handle: &OutputStreamHandle,
    state: &mut AudioState,
    path: PathBuf,
) -> Result<Option<Duration>, String> {
    let file = File::open(&path).map_err(|e| format!("Could not open file {:?}: {e}", path))?;
    let reader = BufReader::new(file);

    let source = Decoder::new(reader).map_err(|e| format!("Audio decode error: {e}"))?;
    let duration = source.total_duration();

    let sink = Sink::try_new(handle).map_err(|e| format!("Sink creation error: {e}"))?;
    sink.append(source);

    state.sink = Some(sink);
    state.apply_volume();

    Ok(duration)
}
