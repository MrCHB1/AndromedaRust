use std::{collections::{HashMap, HashSet, VecDeque}, sync::{Arc, Mutex}};

use eframe::{
    egui::{self, Color32, RichText, Stroke, Ui},
    egui_glow::CallbackFn,
    glow::HasContext,
};

use crate::{app::{custom_widgets::NumericField, main_window::MainWindow}, editor::{actions::{EditorAction, EditorActions}, util::{bin_search_notes, get_min_max_keys_in_selection, get_min_max_ticks_in_selection, manipulate_note_lengths, manipulate_note_ticks, move_element, MIDITick}}, midi::events::note::Note};

// modular edit_function
pub enum EditFunction {
    FlipX(Vec<usize>),
    FlipY(Vec<usize>), // key-wise. the easiest to manipulate lol
    Stretch(Vec<usize>, f32),
    //               tick  snap   max
    //               len   id     tick len
    Chop(Vec<usize>, bool, usize, u32)
}

#[derive(Default)]
pub struct EditFunctions {

}

impl EditFunctions {
    pub fn new() -> Self {
        Self {}
    }

    pub fn apply_function(&mut self, notes: &mut Vec<Note>, sel_note_ids: &mut Vec<usize>, func: EditFunction, curr_track: u16, curr_channel: u8, editor_actions: &mut EditorActions) {
        match func {
            EditFunction::FlipX(note_ids) => {
                let (min_tick, max_tick) = {
                    let first_note = notes[sel_note_ids[0]];
                    let last_note = notes[sel_note_ids[sel_note_ids.len() - 1]];
                    (first_note.start, last_note.start)
                };

                let (old_ids, new_ids, changed_positions) = manipulate_note_ticks(notes, &note_ids, |note_start| {
                    max_tick - note_start + min_tick
                });

                // update selected note ids to the new ids to prevent index invalidation
                // *sel_note_ids = new_ids.clone();
                println!("{:?}", new_ids);
                *sel_note_ids = new_ids.clone();

                editor_actions.register_action(EditorAction::NotesMove(old_ids, new_ids, changed_positions, curr_track as u32 * 16 + curr_channel as u32, true));
            },
            EditFunction::FlipY(note_ids) => {
                let mut changed_positions = Vec::new();
                let (min_key, max_key) = get_min_max_keys_in_selection(notes, &note_ids).unwrap_or_default();

                for id in note_ids.iter() {
                    let note = &mut notes[*id];
                    let old_key = note.key;
                    let new_key = max_key - old_key + min_key;

                    note.key = new_key;
                    changed_positions.push((0, new_key as i16 - old_key as i16));
                }
                editor_actions.register_action(EditorAction::NotesMoveImmediate(note_ids, changed_positions, curr_track as u32 * 16 + curr_channel as u32));
            },
            EditFunction::Stretch(note_ids, factor) => {
                if factor == 1.0 { return; }

                // and because im lazy we change the lengths first
                let changed_lengths = manipulate_note_lengths(notes, &note_ids, |note_length| {
                    (note_length as f32 * factor).round() as MIDITick
                });

                let (old_ids, new_ids, changed_positions) = manipulate_note_ticks(notes, &note_ids, |note_start| {
                    (note_start as f32 * factor).round() as MIDITick
                });

                // update selected note ids to the new ids to prevent index invalidation
                *sel_note_ids = new_ids.clone();

                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::NotesMove(old_ids, new_ids, changed_positions, curr_track as u32 * 16 + curr_channel as u32, true),
                    EditorAction::LengthChange(note_ids, changed_lengths, curr_track as u32 * 16 + curr_channel as u32),
                ]));
            },
            EditFunction::Chop(note_ids, use_tick_lens, snap_id, max_len) => {

            }
        }
    }
}

// the dialogues for these functions
pub struct EFStretchDialog {
    pub stretch_factor: NumericField<f32>,
    pub is_shown: bool
}

impl Default for EFStretchDialog {
    fn default() -> Self {
        Self { stretch_factor: NumericField::new(1.0, Some(0.0), None), is_shown: false }
    }
}

impl EFStretchDialog {
    pub fn show(&mut self) { self.is_shown = true; }
    pub fn close(&mut self) { self.is_shown = false; }
}

pub struct EFChopDialog {
    pub use_tick_lens: bool,
    pub snap_id: usize,
    pub target_tick_len: NumericField<MIDITick>,
    pub is_shown: bool,
}

impl Default for EFChopDialog {
    fn default() -> Self {
        Self { use_tick_lens: false, snap_id: 0, target_tick_len: NumericField::new(240, Some(1), Some(MIDITick::MAX.into())), is_shown: false }
    }
}

impl EFChopDialog {
    pub fn show(&mut self) { self.is_shown = true; }
    pub fn close(&mut self) { self.is_shown = false; }
}