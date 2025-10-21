#![warn(unused)]
// actions.rs - defined for the undo/redo system in the editor.

use std::collections::{VecDeque};

use crate::{editor::util::{MIDITick, SignedMIDITick}, midi::events::{channel_event::ChannelEvent, note::Note}};

#[derive(Clone)]
pub enum EditorAction {
    // used from pencil
    PlaceNotes(
        Vec<usize>, // note ids,
        Option<Vec<Note>>, // only used when undoing or redoing
        u16 // note group (track)
    ),
    // used from pencil or eraser
    DeleteNotes(
        Vec<usize>, // note ids
        Option<Vec<Note>>, // can be by user or from undo/redo
        u16 // note group (track)
    ),
    LengthChange(Vec<usize>, Vec<SignedMIDITick>, u16), // this action stores the change in length of notes.
    VelocityChange(Vec<usize>, Vec<i8>, u16),
    ChannelChange(Vec<usize>, Vec<i8>, u16),
    NotesMove(Vec<usize>, Vec<(SignedMIDITick, i16)>, u16, bool), // stores change in tick and key. last bool is if we should update selected notes ids
    NotesMoveImmediate(Vec<usize>, Vec<(SignedMIDITick, i16)>, u16), // stores change in tick and key without keeping track of the old note ids. this is unsafe lol
    Select(Vec<usize>, u16), // pretty straightforward
    Deselect(Vec<usize>, u16),
    Duplicate(Vec<usize>, MIDITick, u16, u16), // (note_ids, paste_tick, source track/channel, destination track/channel)
    AddMeta(Vec<usize>),
    DeleteMeta(Vec<usize>),
    AddTrack(
        u16, // index of the track that got added
        Option<VecDeque<(Vec<Note>, Vec<ChannelEvent>)>>, // only used for undoing/redoing
        bool, // if this track is the last track
    ),
    RemoveTrack(
        u16, // index of the track that got removed
        Option<VecDeque<(Vec<Note>, Vec<ChannelEvent>)>>, // only used for undoing/redoing,
        bool // if this track is the last track
    ),
    SwapTracks(u16, u16),
    Bulk(Vec<EditorAction>) // for bulk actions
}

/*impl Default for EditorAction {
    fn default() -> Self {
        EditorAction::Nil
    }
}*/

pub struct EditorActions {
    actions: VecDeque<EditorAction>,
    max_actions: u16,
    undo_depth: u16,
}

impl Default for EditorActions {
    fn default() -> Self {
        Self { actions: VecDeque::new(), max_actions: 10, undo_depth: 0 }
    }
}

impl EditorActions {
    pub fn new(max_actions: u16) -> Self {
        Self {
            actions: VecDeque::with_capacity(max_actions as usize),
            max_actions,
            undo_depth: 0
        }
    }

    pub fn register_action(&mut self, action: EditorAction) {
        // if we have previously undid some action(s), remove those actions.
        // if we never removed them then the undo/redo system would no longer be accurate lol
        while self.undo_depth > 0 {
            self.actions.pop_back().unwrap();
            self.undo_depth -= 1;
        }

        // delete the first action if the list of actions is at full capacity
        if self.actions.len() as u16 == self.max_actions { self.actions.pop_front().unwrap(); }

        self.actions.push_back(action);
    }

    // this will basically "invert" the actions, starting from the latest action (front of VecDeque)
    pub fn undo_action(&mut self) -> Option<&mut EditorAction> {
        if !self.get_can_undo() { println!("Nothing to undo"); return None; }

        // increment the number of undo's
        self.undo_depth += 1;

        let lastmost_undo_index = self.actions.len() - self.undo_depth as usize;
        let mut action_to_undo = self.actions.remove(lastmost_undo_index).unwrap();
        // invert this action
        action_to_undo = self.invert_action(action_to_undo);
        // put it back in the deque
        self.actions.insert(lastmost_undo_index, action_to_undo);

        Some(&mut self.actions[lastmost_undo_index])
    }

    // like undo, this will "invert" the actions, but starting from the undo_depth'th last index
    pub fn redo_action(&mut self) -> Option<&mut EditorAction> {
        //if self.undo_depth == 0 { println!("Nothing to redo"); return None; }
        if !self.get_can_redo() { println!("Nothing to redo"); return None; }

        let lastmost_redo_index = self.actions.len() - self.undo_depth as usize;
        let mut action_to_redo = self.actions.remove(lastmost_redo_index).unwrap();
        // invert action
        action_to_redo = self.invert_action(action_to_redo);
        // put it back into the deque
        self.actions.insert(lastmost_redo_index, action_to_redo);

        self.undo_depth -= 1;

        Some(&mut self.actions[lastmost_redo_index])
    }

    pub fn get_can_undo(&self) -> bool {
        if self.actions.is_empty() ||
           self.undo_depth == self.max_actions || self.undo_depth == self.actions.len() as u16 { false }
        else { true }
    }

    pub fn get_can_redo(&self) -> bool {
        if self.undo_depth == 0 { false }
        else { true }
    }

    fn invert_action(&mut self, action: EditorAction) -> EditorAction {
        match action {
            EditorAction::PlaceNotes(note_id, deleted_notes, note_group) => {
                EditorAction::DeleteNotes(note_id, deleted_notes, note_group)
            },
            EditorAction::DeleteNotes(note_id, deleted_notes, note_group) => {
                EditorAction::PlaceNotes(note_id, deleted_notes, note_group)
            },
            EditorAction::LengthChange(note_id, length_delta, note_group) => {
                EditorAction::LengthChange(note_id, length_delta.iter().map(|l| -l).collect(), note_group)
            },
            EditorAction::ChannelChange(note_id, channel_delta, note_group) => {
                EditorAction::ChannelChange(note_id, channel_delta.iter().map(|l| -l).collect(), note_group)
            },
            EditorAction::VelocityChange(note_id, velocity_delta, note_group) => {
                EditorAction::VelocityChange(note_id, velocity_delta.iter().map(|l| -l).collect(), note_group)
            },
            EditorAction::NotesMove(note_id, midi_pos_delta, note_group, update_selected_ids) => {
                EditorAction::NotesMove(note_id, midi_pos_delta.iter().map(|delta| (-delta.0, -delta.1)).collect(), note_group, update_selected_ids)
            },
            EditorAction::NotesMoveImmediate(note_id, midi_pos_delta, note_group) => {
                EditorAction::NotesMoveImmediate(note_id, midi_pos_delta.iter().map(|delta| (-delta.0, -delta.1)).collect(), note_group)
            },
            EditorAction::Select(note_id, note_group) => {
                EditorAction::Deselect(note_id, note_group)
            },
            EditorAction::Deselect(note_id, note_group) => {
                EditorAction::Select(note_id, note_group)
            },
            EditorAction::Duplicate(note_id, _, _, note_group) => { // clever hack >:3
                EditorAction::DeleteNotes(note_id, None, note_group)
            },
            EditorAction::AddMeta(meta_ids) => {
                EditorAction::DeleteMeta(meta_ids)
            },
            EditorAction::DeleteMeta(meta_ids) => {
                EditorAction::AddMeta(meta_ids)
            },
            EditorAction::AddTrack(track, deleted_tracks, last_track) => {
                EditorAction::RemoveTrack(track, deleted_tracks, last_track)
            },
            EditorAction::RemoveTrack(track, deleted_tracks, last_track) => {
                EditorAction::AddTrack(track, deleted_tracks, last_track)
            },
            EditorAction::SwapTracks(track_1, track_2) => {
                EditorAction::SwapTracks(track_1, track_2)
            },
            EditorAction::Bulk(actions) => {
                {
                    let mut inv_actions = Vec::new();
                    for action in actions {
                        inv_actions.push(self.invert_action(action));
                    }
                    inv_actions.reverse();
                    EditorAction::Bulk(inv_actions)
                }
            }
        }
    }

    pub fn clear_actions(&mut self) {
        self.actions.clear();
        self.undo_depth = 0;
    }
}