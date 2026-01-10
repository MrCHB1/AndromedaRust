use crate::{editor::{util::MIDITick}};

// zoom limits
pub const PR_ZOOM_TICKS_MIN: MIDITick = 48;
pub const PR_ZOOM_TICKS_MAX: MIDITick = 384000;
pub const TV_ZOOM_TICKS_MIN: MIDITick = 38400;
pub const TV_ZOOM_TICKS_MAX: MIDITick = 384000;

pub const GLOBAL_ZOOM_FACTOR: f32 = 1.5;

pub struct PianoRollNavigation {
    pub tick_pos: f32,
    pub key_pos: f32,
    pub zoom_ticks: f32,
    pub zoom_keys: f32,

    pub curr_track: u16,

    pub tick_pos_smoothed: f32,
    pub key_pos_smoothed: f32,
    pub zoom_ticks_smoothed: f32,
    pub zoom_keys_smoothed: f32
}

impl Default for PianoRollNavigation {
    fn default() -> Self {
        Self {
            tick_pos: 0.0,
            key_pos: 21.0,
            zoom_ticks: 7680.0,
            zoom_keys: 88.0,

            curr_track: 0,

            tick_pos_smoothed: 0.0,
            key_pos_smoothed: 21.0,
            zoom_ticks_smoothed: 7680.0,
            zoom_keys_smoothed: 88.0
        }
    }
}

impl PianoRollNavigation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn change_tick_pos(&mut self, tick_pos: f32, mut change_fn: impl FnMut(f32)) {
        self.tick_pos = tick_pos;
        change_fn(self.tick_pos);
    }

    pub fn update_smoothed_values(&mut self) {
        self.tick_pos_smoothed = (self.tick_pos * 0.1) + (self.tick_pos_smoothed * 0.9);
        self.key_pos_smoothed = (self.key_pos * 0.1) + (self.key_pos_smoothed * 0.9);
        self.zoom_ticks_smoothed = (self.zoom_ticks * 0.1) + (self.zoom_ticks_smoothed * 0.9);
        self.zoom_keys_smoothed = (self.zoom_keys * 0.1) + (self.zoom_keys_smoothed * 0.9);
    }

    /// If the smoothed values aren't close enough to the actual underlying values, returns true.
    pub fn smoothed_values_needs_update(&self) -> bool {
        let tick_diff = (self.tick_pos - self.tick_pos_smoothed).abs();
        let key_diff = (self.key_pos - self.key_pos_smoothed).abs();
        let zoom_tick_diff = (self.zoom_ticks - self.zoom_ticks_smoothed).abs();
        let zoom_key_diff = (self.zoom_keys - self.zoom_keys_smoothed).abs();
        tick_diff > 0.001 || key_diff > 0.001 || zoom_tick_diff > 0.001 || zoom_key_diff > 0.001
    }

    pub fn zoom_ticks_by(&mut self, fac: f32) {
        let mut new_zoom_ticks = self.zoom_ticks * fac;
        if new_zoom_ticks < PR_ZOOM_TICKS_MIN as f32 {
            new_zoom_ticks = PR_ZOOM_TICKS_MIN as f32;
        }
        if new_zoom_ticks > PR_ZOOM_TICKS_MAX as f32 {
            new_zoom_ticks = PR_ZOOM_TICKS_MAX as f32;
        }
        self.zoom_ticks = new_zoom_ticks;
    }

    pub fn zoom_keys_by(&mut self, fac: f32) {
        let mut new_zoom_keys = self.zoom_keys * fac;
        if new_zoom_keys < 12.0 { new_zoom_keys = 12.0; }
        if new_zoom_keys > 128.0 { new_zoom_keys = 128.0; }

        let view_top = self.key_pos + self.zoom_keys;
        self.zoom_keys = new_zoom_keys;
        let view_top_new = self.key_pos + self.zoom_keys;
        let view_top_delta = view_top_new - view_top;

        if view_top_new > 128.0 { self.key_pos -= view_top_delta; }
        if self.key_pos < 0.0 {
            self.key_pos = 0.0;
        }
    }
}

pub struct TrackViewNavigation {
    pub tick_pos: f32,
    pub track_pos: f32,
    pub zoom_ticks: f32,
    pub zoom_tracks: f32,

    pub tick_pos_smoothed: f32,
    pub track_pos_smoothed: f32,
    pub zoom_ticks_smoothed: f32,
    pub zoom_tracks_smoothed: f32
}

impl Default for TrackViewNavigation {
    fn default() -> Self {
        Self {
            tick_pos: 0.0,
            track_pos: 0.0,
            zoom_ticks: 3840.0 * 10.0,
            zoom_tracks: 10.0,

            tick_pos_smoothed: 0.0,
            track_pos_smoothed: 0.0,
            zoom_ticks_smoothed: 3840.0 * 10.0,
            zoom_tracks_smoothed: 10.0,
        }
    }
}

impl TrackViewNavigation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn change_tick_pos(&mut self, tick_pos: f32, mut change_fn: impl FnMut(f32)) {
        self.tick_pos = tick_pos;
        change_fn(self.tick_pos);
    }

    pub fn update_smoothed_values(&mut self) {
        self.tick_pos_smoothed = (self.tick_pos * 0.1) + (self.tick_pos_smoothed * 0.9);
        self.track_pos_smoothed = (self.track_pos * 0.1) + (self.track_pos_smoothed * 0.9);
        self.zoom_ticks_smoothed = (self.zoom_ticks * 0.1) + (self.zoom_ticks_smoothed * 0.9);
        self.zoom_tracks_smoothed = (self.zoom_tracks * 0.1) + (self.zoom_tracks_smoothed * 0.9);
    }

    /// If the smoothed values aren't close enough to the actual underlying values, returns true.
    pub fn smoothed_values_needs_update(&self) -> bool {
        let tick_diff = (self.tick_pos - self.tick_pos_smoothed).abs();
        let track_diff = (self.track_pos - self.track_pos_smoothed).abs();
        tick_diff > 0.001 || track_diff > 0.001
    }

    pub fn zoom_ticks_by(&mut self, fac: f32) {
        let mut new_zoom_ticks = self.zoom_ticks * fac;
        if new_zoom_ticks < TV_ZOOM_TICKS_MIN as f32 {
            new_zoom_ticks = TV_ZOOM_TICKS_MIN as f32;
        }
        if new_zoom_ticks > TV_ZOOM_TICKS_MAX as f32 {
            new_zoom_ticks = TV_ZOOM_TICKS_MAX as f32;
        }
        self.zoom_ticks = new_zoom_ticks;
    }

    pub fn zoom_tracks_by(&mut self, fac: f32) {
        let mut new_zoom_tracks = self.zoom_tracks * fac;
        if new_zoom_tracks < 10.0 { new_zoom_tracks = 10.0; }
        if new_zoom_tracks > 64.0 { new_zoom_tracks = 64.0; }
        self.zoom_tracks = new_zoom_tracks;
    }
}