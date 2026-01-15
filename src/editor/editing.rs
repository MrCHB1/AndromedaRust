use std::collections::HashMap;

use crate::{editor::util::{MIDITick, SignedMIDITick}, midi::events::note::Note};

pub mod note_editing;
pub mod meta_editing;
pub mod track_editing;
pub mod lua_note_editing;
pub mod data_editing;

/// Contains information about what notes were copied
#[derive(Default)]
pub struct SharedClipboard {
    notes_clipboard_map: HashMap<u16, Vec<Note>>,
    pub offset_from_playhead: SignedMIDITick,
}

impl SharedClipboard {
    /// This will override the current clipboard.
    pub fn move_notes_to_clipboard(&mut self, notes: Vec<Note>, track: u16, clear_clipboard: bool) {
        let clipboard_map = &mut self.notes_clipboard_map;
        if clear_clipboard { clipboard_map.clear(); }
        clipboard_map.insert(track, notes);
    }

    /// Overrides the current clipboard but allows for multiple notes to be moved to the clipboard
    pub fn move_multi_notes_to_clipboard(&mut self, notes: Vec<Vec<Note>>, tracks: Vec<u16>) {
        let clipboard_map = &mut self.notes_clipboard_map;
        clipboard_map.clear();

        for (notes, track) in notes.into_iter().zip(tracks) {
            clipboard_map.insert(track, notes);
        }
    }

    /// Retrieves notes from the clipboard, cloning the notes from the clipboard.
    /// Returns a track-notes pair.
    pub fn get_notes_from_clipboard(&self) -> Vec<(u16, Vec<Note>)> {
        let mut retrieved = Vec::with_capacity(self.notes_clipboard_map.len());
        for (&track, notes) in self.notes_clipboard_map.iter() {
            retrieved.push((track, notes.clone()));
        }
        retrieved.sort_by_key(|t| t.0);
        retrieved
    }
    
    pub fn clear_clipboard(&mut self) {
        self.notes_clipboard_map.clear();
    }

    pub fn is_clipboard_empty(&self) -> bool {
        self.notes_clipboard_map.is_empty()
    }

    pub fn get_clipboard_start_tick(&self) -> MIDITick {
        let mut start_tick = MIDITick::MAX;

        for (_, notes) in self.notes_clipboard_map.iter() {
            let first_note_tick = (*notes).first().unwrap().start();
            if first_note_tick <= start_tick {
                start_tick = first_note_tick;
            }
        }

        start_tick
    }
}

#[derive(Default)]
pub struct SharedSelectedNotes {
    selected_notes_hash: HashMap<u16, Vec<usize>>
}

impl SharedSelectedNotes {
    pub fn get_active_selected_tracks(&self) -> Vec<u16> {
        let mut tracks: Vec<u16> = self.selected_notes_hash.keys().map(|i| *i).collect();
        tracks.sort();
        tracks
    }

    pub fn get_selected_ids_in_track(&self, track: u16) -> Option<&Vec<usize>> {
        self.selected_notes_hash.get(&track)
    }

    pub fn get_selected_ids_mut(&mut self, track: u16) -> &mut Vec<usize> {
        self.selected_notes_hash.entry(track).or_insert(vec![])
    }

    pub fn get_selected(&self) -> Vec<(u16, &Vec<usize>)> {
        let mut retrieved = Vec::with_capacity(self.selected_notes_hash.len());
        for (&track, note_ids) in self.selected_notes_hash.iter() {
            retrieved.push((track, note_ids))
        }
        retrieved
    }

    pub fn set_selected_in_track(&mut self, ids: Vec<usize>, track: u16) {
        self.selected_notes_hash.insert(track, ids);
    }

    pub fn add_selected_to_track(&mut self, ids: Vec<usize>, track: u16) {
        let mut selected = self.take_selected_from_track(track);
        selected.extend(ids);
        selected.sort_unstable();
        selected.dedup();
        self.selected_notes_hash.insert(track, selected);
    }

    pub fn take_selected_from_track(&mut self, track: u16) -> Vec<usize> {
        std::mem::take(&mut self.selected_notes_hash.entry(track).or_default())
    }

    pub fn take_selected_from_all(&mut self) -> Vec<(u16, Vec<usize>)> {
        let mut result = Vec::with_capacity(self.selected_notes_hash.len());
        for (track, ids) in self.selected_notes_hash.drain() {
            result.push((track, ids));
        }
        result
    }

    pub fn clear_selected(&mut self) {
        self.selected_notes_hash.clear();
    }

    pub fn is_any_note_selected(&self) -> bool {
        if self.selected_notes_hash.is_empty() { false }
        else {
            for (_, ids) in self.selected_notes_hash.iter() {
                if !ids.is_empty() { return true }
            }
            false
        }
    }

    pub fn selected_ids_in_track_contains(&self, track: u16, id: usize) -> bool {
        match self.get_selected_ids_in_track(track) {
            Some(ids) => ids.contains(&id),
            None => false
        }
    }

    pub fn get_num_selected_in_track(&self, track: u16) -> usize {
        match self.get_selected_ids_in_track(track) {
            Some(ids) => ids.len(),
            None => 0
        }
    }
}