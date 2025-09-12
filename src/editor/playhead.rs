use crate::{audio::event_playback::PlaybackManager, editor::util::MIDITick};
use std::sync::{Arc, Mutex};

pub struct Playhead {
    pub start_tick: MIDITick,
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>
}

impl Default for Playhead {
    fn default() -> Self {
        Self {
            start_tick: 0,
            playback_manager: None
        }
    }
}

impl Playhead { 
    pub fn new(start_tick: MIDITick, playback_manager: &Arc<Mutex<PlaybackManager>>) -> Self {
        let mut s = Self {
            start_tick: 0,
            playback_manager: Some(playback_manager.clone())
        };

        s.set_start(start_tick);
        s
    }

    pub fn set_start(&mut self, start_tick: MIDITick) {
        self.start_tick = start_tick;
        if let Some(playback_manager) = self.playback_manager.as_mut() {
            let mut playback_manager = playback_manager.lock().unwrap();
            playback_manager.navigate_to(start_tick);
        }
    }
}