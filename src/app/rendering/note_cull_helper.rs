use std::sync::{Arc, RwLock};

use crate::midi::midi_track::MIDITrack;

// Helper file for computing note culling.
#[derive(Default)]
pub struct NoteCullHelper {
    tracks: Arc<RwLock<Vec<MIDITrack>>>,

    first_render: Vec<usize>,
    end_render: Vec<usize>,
    last_start: Vec<usize>,

    last_time: Vec<f32>,
    last_zoom: Vec<f32>,

    // cull_updated: bool,
}

impl NoteCullHelper {
    pub fn new(tracks: &Arc<RwLock<Vec<MIDITrack>>>) -> Self {
        let tracks_ = tracks.read().unwrap();
        let n_tracks = tracks_.len();
        let (first_render, end_render, last_start) = (vec![0; n_tracks], vec![0; n_tracks], vec![0; n_tracks]);

        Self {
            first_render,
            end_render,
            last_start,
            tracks: tracks.clone(),
            last_time: vec![f32::NAN; n_tracks],
            last_zoom: vec![f32::NAN; n_tracks],

            // cull_updated: false,
        }
    }

    pub fn update_cull_for_track(&mut self, track: u16, time: f32, zoom: f32, force: bool) {
        self.sync_cull_array_lengths();

        let track = track as usize;
        // let notes = self.notes.read().unwrap();
        let tracks = self.tracks.read().unwrap();
        let notes = (*tracks)[track as usize].get_notes();
        
        if notes.is_empty() { return; }

        if force {
            self.first_render[track] = 0;
            self.end_render[track] = 0;
        }

        // if self.last_time[track] == time && self.last_zoom[track] == zoom { return; }

        let mut n_off = self.first_render[track];
        if self.last_time[track] > time {
            if n_off == 0 {
                for note in &notes[0..notes.len()] {
                    if note.end() as f32 > time { break; }
                    n_off += 1;
                }
            } else {
                if n_off > notes.len() { // hmm.. we should recalculate the cull then
                    self.first_render[track] = 0;
                    self.last_start[track] = 0;
                    return;
                 }

                for note in notes[0..n_off].iter().rev() {
                    if (note.end() as f32) <= time { break; }
                    n_off -= 1;
                }
            }

            self.first_render[track] = n_off;
        } else if self.last_time[track] < time {
            for note in &notes[n_off..notes.len()] {
                if note.end() as f32 > time { break; }
                n_off += 1;
            }
            self.first_render[track] = n_off;
        }

        /*let mut e = n_off;
        for note in &notes[n_off..notes.len()] {
            if note.start() as f32 > time + zoom { break; }
            e += 1;
        }*/

        // TEST: Binary search instead of linear search
        let note_end = n_off + notes[n_off..].partition_point(|note| note.start() as f32 <= time + zoom);
        self.end_render[track] = note_end;

        self.last_time[track] = time;
        self.last_zoom[track] = zoom;
    }

    /// Returns the note rendering window.
    pub fn get_track_cull_range(&mut self, track: u16) -> (usize, usize) {
        (self.first_render[track as usize], self.end_render[track as usize])
    }

    pub fn sync_cull_array_lengths(&mut self) {
        let tracks = self.tracks.read().unwrap();
        let n_tracks = tracks.len();

        if self.last_start.len() != n_tracks {
            self.last_start = vec![0; n_tracks];
            self.first_render = vec![0; n_tracks];
            self.end_render = vec![0; n_tracks];
            self.last_time = vec![f32::NAN; n_tracks];
            self.last_zoom = vec![f32::NAN; n_tracks];
        }
    }
}