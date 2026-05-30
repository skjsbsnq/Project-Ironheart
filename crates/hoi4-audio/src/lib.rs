//! Audio runtime for music and UI sounds.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use hoi4_paths::PathConfig;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

#[derive(Debug, Clone, Copy)]
pub struct AudioVolumes {
    pub master: f32,
    pub music: f32,
    pub ui: f32,
}

impl Default for AudioVolumes {
    fn default() -> Self {
        Self {
            master: 0.80,
            music: 0.65,
            ui: 0.85,
        }
    }
}

impl AudioVolumes {
    pub fn music_effective(&self) -> f32 {
        (self.master * self.music).clamp(0.0, 1.0)
    }

    pub fn ui_effective(&self) -> f32 {
        (self.master * self.ui).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
pub struct MusicTrack {
    pub name: String,
    pub path: PathBuf,
    pub asset_volume: f32,
}

pub struct MusicPlayer {
    playlist: Vec<MusicTrack>,
    current_index: usize,
    playing: Arc<AtomicBool>,
    volumes: AudioVolumes,
    _stream: Option<OutputStream>,
    sink: Option<Sink>,
}

impl MusicPlayer {
    pub fn new() -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };
        let sink = handle.as_ref().and_then(|h| Sink::try_new(h).ok());
        Self {
            playlist: Vec::new(),
            current_index: 0,
            playing: Arc::new(AtomicBool::new(false)),
            volumes: AudioVolumes::default(),
            _stream: stream,
            sink,
        }
    }

    pub fn load_playlist(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|e| e == "ogg").unwrap_or(false) {
                    let name = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("track")
                        .to_owned();
                    self.playlist.push(MusicTrack {
                        name,
                        path: p,
                        asset_volume: 1.0,
                    });
                }
            }
        }
        self.playlist.sort_by(|a, b| a.path.cmp(&b.path));
    }

    pub fn load_playlist_from_paths(&mut self, paths: &PathConfig) {
        for dir in paths.find_all("music") {
            self.load_playlist(&dir);
        }
    }

    pub fn load_assets<'a, I>(&mut self, game_root: &Path, entries: I)
    where
        I: IntoIterator<Item = (&'a str, &'a str, f32)>,
    {
        for (name, file, volume) in entries {
            let full = game_root.join(file);
            if full.exists() {
                self.playlist.push(MusicTrack {
                    name: name.to_owned(),
                    path: full,
                    asset_volume: volume.clamp(0.0, 4.0),
                });
            }
        }
    }

    pub fn load_assets_from_paths<'a, I>(&mut self, paths: &PathConfig, entries: I)
    where
        I: IntoIterator<Item = (&'a str, &'a str, f32)>,
    {
        for (name, file, volume) in entries {
            if let Some(full) = paths.find(file) {
                self.playlist.push(MusicTrack {
                    name: name.to_owned(),
                    path: full,
                    asset_volume: volume.clamp(0.0, 4.0),
                });
            }
        }
    }

    pub fn track_count(&self) -> usize {
        self.playlist.len()
    }

    pub fn current_name(&self) -> Option<&str> {
        if self.playlist.is_empty() {
            None
        } else {
            Some(&self.playlist[self.current_index % self.playlist.len()].name)
        }
    }

    pub fn set_volumes(&mut self, vols: AudioVolumes) {
        self.volumes = vols;
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volumes.music_effective() * self.current_asset_volume());
        }
    }

    pub fn volumes(&self) -> AudioVolumes {
        self.volumes
    }

    fn current_asset_volume(&self) -> f32 {
        if self.playlist.is_empty() {
            1.0
        } else {
            self.playlist[self.current_index % self.playlist.len()].asset_volume
        }
    }

    pub fn play(&mut self) {
        let Some(sink) = &self.sink else { return };
        if self.playlist.is_empty() {
            return;
        }
        let idx = self.current_index % self.playlist.len();
        let track = &self.playlist[idx];
        if let Ok(file) = std::fs::File::open(&track.path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(source) = rodio::Decoder::new(reader) {
                sink.stop();
                sink.set_volume(self.volumes.music_effective() * track.asset_volume);
                sink.append(source);
                sink.play();
                self.playing.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn next(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.playlist.len();
        self.play();
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub fn current_track(&self) -> usize {
        self.current_index
    }

    pub fn tick_autoadvance(&mut self) {
        if let Some(sink) = &self.sink {
            if self.playing.load(Ordering::Relaxed) && sink.empty() {
                self.next();
            }
        }
    }
}

impl Default for MusicPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSound {
    Click,
    OptionClick,
    Hover,
    PageFlip,
    EventPopup,
    WorldNews,
    WorldDefeat,
    WarDeclaration,
}

impl UiSound {
    pub fn all() -> &'static [Self] {
        &[
            Self::Click,
            Self::OptionClick,
            Self::Hover,
            Self::PageFlip,
            Self::EventPopup,
            Self::WorldNews,
            Self::WorldDefeat,
            Self::WarDeclaration,
        ]
    }

    pub fn vanilla_candidates(self) -> &'static [&'static str] {
        match self {
            UiSound::Click => &[
                "sound/menu/click_default.wav",
                "sound/menu/click_ok.wav",
                "sound/menu/click_submenu.wav",
            ],
            UiSound::OptionClick => &["sound/menu/click_ok.wav", "sound/menu/click_default.wav"],
            UiSound::Hover => &[
                "sound/menu/click_mouse_over_01.wav",
                "sound/menu/click_mouse_over_02.wav",
                "sound/menu/click_mouse_over_03.wav",
            ],
            UiSound::PageFlip => &[
                "sound/menu/menu_open_window.wav",
                "sound/menu/click_window_open.wav",
                "sound/menu/click_expand.wav",
            ],
            UiSound::EventPopup => &[
                "sound/menu/event_popup_01.wav",
                "sound/menu/operative_event_close_01.wav",
                "sound/menu/click_expand.wav",
                "sound/menu/menu_open_window.wav",
            ],
            UiSound::WorldNews => &[
                "sound/menu/event_popup_01.wav",
                "sound/menu/menu_open_window.wav",
                "sound/menu/click_expand.wav",
            ],
            UiSound::WorldDefeat => &[
                "sound/menu/world_news.wav",
                "sound/menu/event_popup_01.wav",
                "sound/menu/menu_open_window.wav",
            ],
            UiSound::WarDeclaration => &[
                "sound/menu/player_declare_war_01.wav",
                "sound/menu/enemy_declare_war_01.wav",
                "sound/menu/high_alert_01.wav",
                "sound/menu/event_popup_01.wav",
            ],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UiSound::Click => "click",
            UiSound::OptionClick => "option_click",
            UiSound::Hover => "hover",
            UiSound::PageFlip => "page_flip",
            UiSound::EventPopup => "event_popup",
            UiSound::WorldNews => "world_news",
            UiSound::WorldDefeat => "world_defeat",
            UiSound::WarDeclaration => "war_declaration",
        }
    }
}

pub struct UiSoundBank {
    handle: Option<OutputStreamHandle>,
    _stream: Option<OutputStream>,
    samples: [Option<Arc<Vec<u8>>>; 8],
    sinks: Vec<Sink>,
    next_sink: usize,
    volumes: AudioVolumes,
}

impl UiSoundBank {
    pub fn new() -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };
        let sinks = handle
            .as_ref()
            .map(|h| {
                (0..4)
                    .filter_map(|_| Sink::try_new(h).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Self {
            handle,
            _stream: stream,
            samples: [None, None, None, None, None, None, None, None],
            sinks,
            next_sink: 0,
            volumes: AudioVolumes::default(),
        }
    }

    pub fn load_vanilla(&mut self, game_root: &Path) -> usize {
        let mut loaded = 0;
        for s in UiSound::all() {
            let idx = sound_index(*s);
            for cand in s.vanilla_candidates() {
                let full = game_root.join(cand);
                if let Ok(data) = std::fs::read(&full) {
                    self.samples[idx] = Some(Arc::new(data));
                    loaded += 1;
                    break;
                }
            }
        }
        loaded
    }

    pub fn load_vanilla_from_paths(&mut self, paths: &PathConfig) -> usize {
        let mut loaded = 0;
        for s in UiSound::all() {
            let idx = sound_index(*s);
            for cand in s.vanilla_candidates() {
                let Some(full) = paths.find(cand) else {
                    continue;
                };
                if let Ok(data) = std::fs::read(&full) {
                    self.samples[idx] = Some(Arc::new(data));
                    loaded += 1;
                    break;
                }
            }
        }
        loaded
    }

    pub fn play(&mut self, sound: UiSound) -> bool {
        if self.handle.is_none() || self.sinks.is_empty() {
            return false;
        }
        let Some(data) = &self.samples[sound_index(sound)] else {
            return false;
        };
        let cursor = std::io::Cursor::new(data.clone().to_vec());
        if let Ok(decoder) = rodio::Decoder::new(std::io::BufReader::new(cursor)) {
            let sink = &self.sinks[self.next_sink % self.sinks.len()];
            self.next_sink = (self.next_sink + 1) % self.sinks.len();
            sink.set_volume(self.volumes.ui_effective());
            sink.append(decoder.convert_samples::<f32>());
            return true;
        }
        false
    }

    pub fn play_with_fallback(&mut self, sound: UiSound, fallback: UiSound) -> bool {
        if self.play(sound) {
            return true;
        }
        self.play(fallback)
    }

    pub fn set_volumes(&mut self, vols: AudioVolumes) {
        self.volumes = vols;
        for sink in &self.sinks {
            sink.set_volume(self.volumes.ui_effective());
        }
    }

    pub fn loaded_count(&self) -> usize {
        self.samples.iter().filter(|s| s.is_some()).count()
    }

    pub fn has_loaded(&self, sound: UiSound) -> bool {
        self.samples[sound_index(sound)].is_some()
    }
}

impl Default for UiSoundBank {
    fn default() -> Self {
        Self::new()
    }
}

fn sound_index(sound: UiSound) -> usize {
    match sound {
        UiSound::Click => 0,
        UiSound::OptionClick => 1,
        UiSound::Hover => 2,
        UiSound::PageFlip => 3,
        UiSound::EventPopup => 4,
        UiSound::WorldNews => 5,
        UiSound::WorldDefeat => 6,
        UiSound::WarDeclaration => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_player_not_playing() {
        let p = MusicPlayer::new();
        assert!(!p.is_playing());
        assert_eq!(p.track_count(), 0);
    }

    #[test]
    fn next_wraps_around() {
        let mut p = MusicPlayer::new();
        p.playlist = vec![
            MusicTrack {
                name: "a".into(),
                path: PathBuf::from("a.ogg"),
                asset_volume: 1.0,
            },
            MusicTrack {
                name: "b".into(),
                path: PathBuf::from("b.ogg"),
                asset_volume: 1.0,
            },
        ];
        assert_eq!(p.current_track(), 0);
        p.next();
        assert_eq!(p.current_track(), 1);
        p.next();
        assert_eq!(p.current_track(), 0);
    }

    #[test]
    fn volumes_master_zero_silences_all() {
        let v = AudioVolumes {
            master: 0.0,
            music: 1.0,
            ui: 1.0,
        };
        assert_eq!(v.music_effective(), 0.0);
        assert_eq!(v.ui_effective(), 0.0);
    }

    #[test]
    fn ui_sound_categories_have_candidates() {
        for s in UiSound::all() {
            assert!(!s.vanilla_candidates().is_empty());
        }
    }
}
