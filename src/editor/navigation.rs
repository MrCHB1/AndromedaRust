pub struct PianoRollNavigation {
    pub tick_pos: f32,
    pub key_pos: f32,
    pub zoom_ticks: f32,
    pub zoom_keys: f32,

    pub curr_track: u16,
    pub curr_channel: u8,

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
            curr_channel: 0,

            tick_pos_smoothed: 0.0,
            key_pos_smoothed: 21.0,
            zoom_ticks_smoothed: 7680.0,
            zoom_keys_smoothed: 88.0,
        }
    }
}

impl PianoRollNavigation {
    pub fn new() -> Self {
        Self {
            tick_pos: 0.0,
            key_pos: 21.0,
            zoom_ticks: 7680.0,
            zoom_keys: 88.0,

            curr_track: 0,
            curr_channel: 0,

            tick_pos_smoothed: 0.0,
            key_pos_smoothed: 21.0,
            zoom_ticks_smoothed: 7680.0,
            zoom_keys_smoothed: 88.0,
        }
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

impl TrackViewNavigation {
    pub fn new() -> Self {
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
}