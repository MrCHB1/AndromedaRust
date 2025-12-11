use std::sync::{Arc, RwLock};

use crate::midi::midi_track::MIDITrack;

// handles muting/soloing tracks
#[derive(Default)]
pub struct TrackMixer {
    tracks: Arc<RwLock<Vec<MIDITrack>>>
}

impl TrackMixer {
    pub fn new(tracks: &Arc<RwLock<Vec<MIDITrack>>>) -> Self {
        Self {
            tracks: tracks.clone()
        }
    }

    /// Sets the mute state for a track.
    pub fn set_track_muted(&mut self, track: u16, muted: bool) {
        let mut tracks = self.tracks.write().unwrap();
        let track = &mut tracks[track as usize];
        track.muted = muted;
    }

    /// Mutes all other tracks except the specified one.
    pub fn solo_track(&mut self, track: u16) {
        let mut tracks = self.tracks.write().unwrap();
        for (t, trk) in tracks.iter_mut().enumerate() {
            trk.muted = t as u16 != track;
        }
    }

    /// Unmutes every track.
    pub fn unmute_all_tracks(&mut self) {
        let mut tracks = self.tracks.write().unwrap();
        for track in tracks.iter_mut() {
            track.muted = false;
        }
    }
}