// actions.rs - defined for the undo/redo system in the editor.

use std::collections::{VecDeque};

use crate::editor::util::{MIDITick, SignedMIDITick};

#[derive(Clone)]
pub enum EditorAction {
    PlaceNotes(Vec<usize>, u32), // used from pencil. PlaceNote(nodeId or noteIndex, track * 16 + channel)
    DeleteNotes(Vec<usize>, u32), // used from either pencil (when holding right mouse button) or eraser. DeleteNote(noteId, track * 16 + channel)
    LengthChange(Vec<usize>, Vec<SignedMIDITick>, u32), // this action stores the change in length of notes.
    NotesMove(Vec<usize>, Vec<usize>, Vec<(SignedMIDITick, i16)>, u32, bool), // stores change in tick and key. last bool is if we should update selected notes ids
    NotesMoveImmediate(Vec<usize>, Vec<(SignedMIDITick, i16)>, u32), // stores change in tick and key without keeping track of the old note ids. this is unsafe lol
    Select(Vec<usize>, u32), // pretty straightforward
    Deselect(Vec<usize>, u32),
    Duplicate(Vec<usize>, MIDITick, u32, u32), // (note_ids, paste_tick, source track/channel, destination track/channel)
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
    pub fn undo_action(&mut self) -> Option<EditorAction> {
        if !self.get_can_undo() { println!("Nothing to undo"); return None; }

        // increment the number of undo's
        self.undo_depth += 1;

        let lastmost_undo_index = self.actions.len() - self.undo_depth as usize;
        let mut action_to_undo = self.actions.remove(lastmost_undo_index).unwrap();
        // invert this action
        action_to_undo = self.invert_action(action_to_undo);
        // put it back in the deque
        self.actions.insert(lastmost_undo_index, action_to_undo.clone());

        Some(action_to_undo)
    }

    // like undo, this will "invert" the actions, but starting from the undo_depth'th last index
    pub fn redo_action(&mut self) -> Option<EditorAction> {
        //if self.undo_depth == 0 { println!("Nothing to redo"); return None; }
        if !self.get_can_redo() { println!("Nothing to redo"); return None; }

        let lastmost_redo_index = self.actions.len() - self.undo_depth as usize;
        let mut action_to_redo = self.actions.remove(lastmost_redo_index).unwrap();
        // invert action
        action_to_redo = self.invert_action(action_to_redo);
        // put it back into the deque
        self.actions.insert(lastmost_redo_index, action_to_redo.clone());

        self.undo_depth -= 1;
        Some(action_to_redo)
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
            EditorAction::PlaceNotes(note_id, note_group) => {
                EditorAction::DeleteNotes(note_id, note_group)
            },
            EditorAction::DeleteNotes(note_id, note_group) => {
                EditorAction::PlaceNotes(note_id, note_group)
            },
            EditorAction::LengthChange(note_id, length_delta, note_group) => {
                EditorAction::LengthChange(note_id, length_delta.iter().map(|l| -l).collect(), note_group)
            },
            EditorAction::NotesMove(note_id, new_note_id, midi_pos_delta, note_group, update_selected_ids) => {
                EditorAction::NotesMove(new_note_id, note_id, midi_pos_delta.iter().map(|delta| (-delta.0, -delta.1)).collect(), note_group, update_selected_ids)
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
                EditorAction::DeleteNotes(note_id, note_group)
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