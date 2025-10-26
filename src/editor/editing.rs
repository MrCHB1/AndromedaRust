use std::collections::HashMap;

use crate::midi::events::note::Note;

pub mod note_editing;
pub mod meta_editing;
pub mod track_editing;
pub mod lua_note_editing;
pub mod data_editing;

/// Contains information about what notes were copied
#[derive(Default)]
pub struct SharedClipboard {
    notes_clipboard_map: HashMap<u16, Vec<Note>>
}

impl SharedClipboard {
    /// This will override the current clipboard.
    pub fn move_notes_to_clipboard(&mut self, notes: Vec<Note>, track: u16) {
        let clipboard_map = &mut self.notes_clipboard_map;
        clipboard_map.clear();
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
        retrieved
    }
    
    pub fn clear_clipboard(&mut self) {
        self.notes_clipboard_map.clear();
    }

    pub fn is_clipboard_empty(&self) -> bool {
        self.notes_clipboard_map.is_empty()
    }
}

#[derive(Default)]
pub struct SharedSelectedNotes {
    selected_notes_hash: HashMap<u16, Vec<usize>>
}

impl SharedSelectedNotes {
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

    pub fn take_selected_from_track(&mut self, track: u16) -> Vec<usize> {
        std::mem::take(&mut self.selected_notes_hash.entry(track).or_default())
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