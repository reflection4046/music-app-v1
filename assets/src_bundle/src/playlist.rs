//! Mobile Track Model & Lofty Tag Extraction
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    RepeatOne,
    RepeatAll,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
    pub format: String,
    pub cover_art: Option<Vec<u8>>,
}

impl Track {
    pub fn from_path(path: PathBuf) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_uppercase();

        let filename_fallback = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Track")
            .to_string();

        let mut title = filename_fallback.clone();
        let mut artist = "Unknown Artist".to_string();
        let mut album = "Unknown Album".to_string();
        let mut duration = Duration::ZERO;
        let mut cover_art = None;

        if let Ok(probe) = Probe::open(&path) {
            if let Ok(tagged_file) = probe.read() {
                duration = tagged_file.properties().duration();

                if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
                    if let Some(t) = tag.title() {
                        if !t.trim().is_empty() {
                            title = t.to_string();
                        }
                    }
                    if let Some(a) = tag.artist() {
                        if !a.trim().is_empty() {
                            artist = a.to_string();
                        }
                    }
                    if let Some(alb) = tag.album() {
                        if !alb.trim().is_empty() {
                            album = alb.to_string();
                        }
                    }
                    if let Some(picture) = tag.pictures().first() {
                        cover_art = Some(picture.data().to_vec());
                    }
                }
            }
        }

        Ok(Self {
            path,
            title,
            artist,
            album,
            duration,
            format: extension,
            cover_art,
        })
    }

    pub fn formatted_duration(&self) -> String {
        format_duration(self.duration)
    }
}

pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

#[derive(Default)]
pub struct Playlist {
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub shuffle_indices: Vec<usize>,
    pub shuffle_pos: usize,
    pub is_shuffle: bool,
    pub repeat_mode: RepeatMode,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
        self.rebuild_shuffle_order();
    }

    pub fn add_tracks(&mut self, new_tracks: Vec<Track>) {
        self.tracks.extend(new_tracks);
        self.rebuild_shuffle_order();
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
        self.shuffle_indices.clear();
        self.shuffle_pos = 0;
    }

    pub fn remove_track(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);
            if let Some(cur) = self.current_index {
                if cur == index {
                    self.current_index = if self.tracks.is_empty() {
                        None
                    } else {
                        Some(cur.min(self.tracks.len() - 1))
                    };
                } else if cur > index {
                    self.current_index = Some(cur - 1);
                }
            }
            self.rebuild_shuffle_order();
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_index.and_then(|idx| self.tracks.get(idx))
    }

    pub fn toggle_shuffle(&mut self) {
        self.is_shuffle = !self.is_shuffle;
        if self.is_shuffle {
            self.rebuild_shuffle_order();
        }
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::Off => RepeatMode::RepeatOne,
            RepeatMode::RepeatOne => RepeatMode::RepeatAll,
            RepeatMode::RepeatAll => RepeatMode::Off,
        };
    }

    fn rebuild_shuffle_order(&mut self) {
        let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
        let mut rng = rand::thread_rng();
        indices.shuffle(&mut rng);
        self.shuffle_indices = indices;
        self.shuffle_pos = 0;
    }

    pub fn get_next_index(&mut self, natural_finish: bool) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }

        if natural_finish && self.repeat_mode == RepeatMode::RepeatOne {
            return self.current_index;
        }

        if self.is_shuffle && !self.shuffle_indices.is_empty() {
            self.shuffle_pos += 1;
            if self.shuffle_pos >= self.shuffle_indices.len() {
                if self.repeat_mode == RepeatMode::RepeatAll {
                    self.rebuild_shuffle_order();
                    self.shuffle_pos = 0;
                } else {
                    return None;
                }
            }
            return self.shuffle_indices.get(self.shuffle_pos).copied();
        }

        match self.current_index {
            Some(cur) => {
                if cur + 1 < self.tracks.len() {
                    Some(cur + 1)
                } else if self.repeat_mode == RepeatMode::RepeatAll {
                    Some(0)
                } else {
                    None
                }
            }
            None => Some(0),
        }
    }

    pub fn get_previous_index(&mut self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }

        if self.is_shuffle && !self.shuffle_indices.is_empty() {
            if self.shuffle_pos > 0 {
                self.shuffle_pos -= 1;
            } else {
                self.shuffle_pos = self.shuffle_indices.len().saturating_sub(1);
            }
            return self.shuffle_indices.get(self.shuffle_pos).copied();
        }

        match self.current_index {
            Some(cur) => {
                if cur > 0 {
                    Some(cur - 1)
                } else {
                    Some(self.tracks.len() - 1)
                }
            }
            None => Some(0),
        }
    }
}

pub fn scan_directory_for_audio<P: AsRef<Path>>(dir: P) -> Vec<Track> {
    let mut tracks = Vec::new();
    let valid_extensions = ["mp3", "wav", "flac", "ogg", "m4a"];

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if valid_extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                    if let Ok(track) = Track::from_path(path.to_path_buf()) {
                        tracks.push(track);
                    }
                }
            }
        }
    }

    tracks
}
