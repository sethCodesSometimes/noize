use crate::config::{Config, ConfigSource};
use crate::state::State;

pub struct Source {
    pub player: mpv::MpvHandler,
    pub name: String,
    pub urls: Vec<String>,
    pub url_index: usize,
    pub volume: i32,
    pub muted: bool,
    pub previous_volume: i32,
    pub is_local: bool,
    pub buffering: bool,
    pub disconnected: bool,
    pub seen_playback: bool,
}

struct SourceSnapshot {
    volume: i32,
    muted: bool,
    previous_volume: i32,
}

pub struct Mixer {
    pub sources: Vec<Source>,
    pub current_index: usize,
    pub max_volume: i32,
    pub volume_step: i32,
    pub global_mute: bool,
    pub dirty: bool,
    saved_states: Option<Vec<SourceSnapshot>>,
    state_path: String,
    last_retry: std::time::Instant,
    retry_interval: std::time::Duration,
}

impl Mixer {
    pub fn new(config_path: &str, fresh: bool) -> anyhow::Result<Self> {
        let config = Config::load(config_path)?;
        let state_path = "state.json".to_string();

        let mut mixer = Mixer {
            sources: Vec::new(),
            current_index: 0,
            max_volume: config.max_volume,
            volume_step: config.volume_step,
            global_mute: false,
            dirty: true,
            saved_states: None,
            state_path,
            last_retry: std::time::Instant::now(),
            retry_interval: std::time::Duration::from_secs(15),
        };

        for src in &config.sources {
            mixer.add_source(src);
        }

        if !fresh {
            mixer.load_state();
        }

        Ok(mixer)
    }

    fn add_source(&mut self, src: &ConfigSource) {
        let is_local = src.urls.iter().all(|u| std::path::Path::new(u).exists());
        let mut builder = mpv::MpvHandlerBuilder::new()
            .expect("failed to create mpv builder");
        builder
            .set_option("ytdl", if is_local { "no" } else { "yes" })
            .expect("failed to set ytdl");
        builder
            .set_option("video", "no")
            .expect("failed to set video=no");
        builder
            .set_option("msg-level", "all=error")
            .expect("failed to set msg-level");
        let player = builder.build().expect("failed to build mpv handler");

        let name = if src.name.is_empty() {
            src.urls[0][..src.urls[0].len().min(40)].to_string()
        } else {
            src.name.clone()
        };

        let source = Source {
            player,
            name,
            urls: src.urls.clone(),
            url_index: 0,
            volume: 0,
            muted: false,
            previous_volume: 0,
            is_local,
            buffering: false,
            disconnected: false,
            seen_playback: false,
        };

        let index = self.sources.len();
        self.sources.push(source);
        if let Err(e) = self.sources[index]
            .player
            .command(&["loadfile", &src.urls[0]])
        {
            eprintln!("[noize] loadfile error for {}: {}", src.urls[0], e);
        }
        self.sources[index]
            .player
            .set_property("volume", 0.0f64)
            .ok();
    }

    fn load_state(&mut self) {
        let state = match State::load(&self.state_path) {
            Ok(Some(s)) => s,
            _ => return,
        };

        for (i, src) in self.sources.iter_mut().enumerate() {
            if let Some(saved) = state.sources.get(i) {
                src.volume = saved.volume;
                src.muted = saved.muted;
                src.previous_volume = saved.previous_volume;
                let perceived = perceived_volume(src.volume, self.max_volume);
                src.player
                    .set_property(
                        "volume",
                        if src.muted { 0.0f64 } else { perceived },
                    )
                    .ok();
            }
        }
        self.current_index = state
            .current_index
            .min(self.sources.len().saturating_sub(1));
        self.dirty = true;
    }

    pub fn save_state(&self) {
        let state = State {
            current_index: self.current_index,
            sources: self
                .sources
                .iter()
                .map(|s| crate::state::SourceState {
                    url: s.urls[0].clone(),
                    volume: s.volume,
                    muted: s.muted,
                    previous_volume: s.previous_volume,
                })
                .collect(),
        };
        state.save(&self.state_path).ok();
    }

    pub fn adjust_volume(&mut self, delta: i32) {
        if self.sources.is_empty() {
            return;
        }
        let src = &mut self.sources[self.current_index];
        let new_vol = (src.volume + delta).clamp(0, self.max_volume);
        src.volume = new_vol;
        src.player
            .set_property("volume", perceived_volume(new_vol, self.max_volume))
            .ok();
        self.dirty = true;
    }

    pub fn toggle_mute(&mut self) {
        if self.sources.is_empty() {
            return;
        }
        let src = &mut self.sources[self.current_index];
        if src.muted {
            src.volume = src.previous_volume;
            src.player
                .set_property("volume", perceived_volume(src.volume, self.max_volume))
                .ok();
            src.muted = false;
        } else {
            src.previous_volume = src.volume;
            src.volume = 0;
            src.player.set_property("volume", 0.0f64).ok();
            src.muted = true;
        }
        self.dirty = true;
    }

    pub fn toggle_global_mute(&mut self) {
        if self.sources.is_empty() {
            return;
        }
        if self.global_mute {
            if let Some(states) = self.saved_states.take() {
                for (i, src) in self.sources.iter_mut().enumerate() {
                    if let Some(saved) = states.get(i) {
                        src.volume = saved.volume;
                        src.muted = saved.muted;
                        src.previous_volume = saved.previous_volume;
                        let perceived = perceived_volume(src.volume, self.max_volume);
                        src.player
                            .set_property(
                                "volume",
                                if src.muted { 0.0f64 } else { perceived },
                            )
                            .ok();
                    }
                }
            }
            self.global_mute = false;
        } else {
            let mut states = Vec::new();
            for src in &mut self.sources {
                states.push(SourceSnapshot {
                    volume: src.volume,
                    muted: src.muted,
                    previous_volume: src.previous_volume,
                });
                src.previous_volume = src.volume;
                src.volume = 0;
                src.muted = true;
                src.player.set_property("volume", 0.0f64).ok();
            }
            self.saved_states = Some(states);
            self.global_mute = true;
        }
        self.dirty = true;
    }

    pub fn scroll_up(&mut self) {
        if !self.sources.is_empty() {
            let len = self.sources.len();
            self.current_index = (self.current_index + len - 1) % len;
            self.dirty = true;
        }
    }

    pub fn scroll_down(&mut self) {
        if !self.sources.is_empty() {
            let len = self.sources.len();
            self.current_index = (self.current_index + 1) % len;
            self.dirty = true;
        }
    }

    pub fn set_volume_direct(&mut self, level: i32) {
        if self.sources.is_empty() {
            return;
        }
        let level = level.clamp(0, self.max_volume);
        let src = &mut self.sources[self.current_index];
        src.muted = false;
        src.volume = level;
        src.player
            .set_property("volume", perceived_volume(level, self.max_volume))
            .ok();
        self.dirty = true;
    }

    fn try_next_url(src: &mut Source) -> bool {
        if src.url_index + 1 < src.urls.len() {
            src.url_index += 1;
            src.player
                .command(&["loadfile", &src.urls[src.url_index], "replace"])
                .ok();
            true
        } else {
            false
        }
    }

    pub fn check_connection_states(&mut self) {
        for src in &mut self.sources {
            if src.is_local {
                if src.disconnected {
                    src.disconnected = false;
                    self.dirty = true;
                }
                continue;
            }

            let idle = src.player.get_property::<bool>("idle-active").ok();
            let paused_for_cache = src
                .player
                .get_property::<bool>("paused-for-cache")
                .ok();
            let core_idle = src.player.get_property::<bool>("core-idle").ok();

            if !src.seen_playback
                && (core_idle == Some(false)
                    || (idle == Some(false) && paused_for_cache.is_some()))
            {
                src.seen_playback = true;
            }

            if src.seen_playback {
                let was_buffering = src.buffering;
                let was_disconnected = src.disconnected;

                match (paused_for_cache, idle) {
                    (Some(true), _) => {
                        src.buffering = true;
                        src.disconnected = false;
                        if !was_buffering {
                            self.dirty = true;
                        }
                    }
                    (_, Some(true)) => {
                        src.buffering = false;
                        if !Self::try_next_url(src) {
                            src.disconnected = true;
                            if !was_disconnected {
                                self.dirty = true;
                            }
                        } else {
                            src.disconnected = false;
                        }
                    }
                    _ => {
                        src.buffering = false;
                        src.disconnected = false;
                        if was_buffering || was_disconnected {
                            self.dirty = true;
                        }
                    }
                }
            }
        }
    }

    pub fn retry_disconnected(&mut self) {
        for src in &mut self.sources {
            if src.disconnected || src.buffering {
                src.url_index = 0;
                src.player
                    .command(&["loadfile", &src.urls[0], "replace"])
                    .ok();
            }
        }
    }

    pub fn tick_retry(&mut self) {
        let now = std::time::Instant::now();
        if now - self.last_retry >= self.retry_interval {
            self.retry_disconnected();
            self.last_retry = now;
        }
    }

    pub fn drain_events(&mut self) {
        for src in &mut self.sources {
            while let Some(_event) = src.player.wait_event(0.0) {}
        }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

pub fn perceived_volume(step: i32, max_volume: i32) -> f64 {
    if max_volume == 0 {
        return 0.0;
    }
    (100.0 * step as f64) / max_volume as f64
}
