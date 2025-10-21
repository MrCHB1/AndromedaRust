use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}};

use eframe::{
    egui::{self, RichText},
};

use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::Dialog, util::image_loader::ImageResources}, editor::{actions::{EditorAction, EditorActions}, util::{bin_search_notes, get_min_max_keys_in_selection, manipulate_note_lengths, manipulate_note_ticks, MIDITick, SignedMIDITick}}, midi::events::note::Note};
use crate::editor::editing::note_editing::NoteEditing;

// modular edit_function
pub enum EditFunction {
    FlipX(Vec<usize>),
    FlipY(Vec<usize>), // key-wise. the easiest to manipulate lol
    Stretch(Vec<usize>, f32),
    //               max
    //               tick len
    Chop(Vec<usize>, MIDITick)
}

#[derive(Default)]
pub struct EditFunctions {

}

impl EditFunctions {
    pub fn apply_function(&mut self, notes: &mut Vec<Note>, sel_note_ids: &mut Vec<usize>, func: EditFunction, curr_track: u16, editor_actions: &mut EditorActions) {
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

                // editor_actions.register_action(EditorAction::NotesMove(old_ids, new_ids, changed_positions, curr_track, true));
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
                editor_actions.register_action(EditorAction::NotesMoveImmediate(note_ids, changed_positions, curr_track));
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
                    // EditorAction::NotesMove(old_ids, new_ids, changed_positions, curr_track, true),
                    EditorAction::LengthChange(note_ids, changed_lengths, curr_track),
                ]));
            },
            EditFunction::Chop(note_ids, max_len) => {
                // to chop, first we go through all selected notes and change their lengths (if applicable)
                let mut changed_lengths = Vec::with_capacity(note_ids.len());

                let mut new_notes = Vec::new();

                for id in note_ids.iter() {
                    let note = &mut notes[*id];
                    let mut remaining_length = note.length();

                    if note.length() > max_len {
                        let old_length = note.length();
                        let new_length = max_len;
                        *note.length_mut() = new_length;
                        remaining_length -= new_length;

                        changed_lengths.push(new_length as SignedMIDITick - old_length as SignedMIDITick);

                        // generate the new chop notes
                        let mut start_time = note.start() + new_length;

                        while remaining_length > 0 {
                            let next_len = remaining_length.min(max_len);
                            let new_note = Note {
                                start: start_time,
                                length: next_len,
                                key: note.key(),
                                channel: note.channel(),
                                velocity: note.velocity()
                            };
                            new_notes.push(new_note);

                            remaining_length -= next_len;
                            start_time += next_len;
                        }
                    }
                }

                // sort chopped in ascending order
                new_notes.sort_by_key(|n| n.start());

                let mut chopped_ids = Vec::new();

                // move new chopped notes to our actual note array
                for note in new_notes.drain(..) {
                    let new_note_idx = bin_search_notes(notes, note.start());
                    chopped_ids.push(new_note_idx);
                    notes.insert(new_note_idx, note);
                }
                
                // sort the chopped ids just in case
                chopped_ids.sort();

                // println!("{:?}", chopped_ids);

                // finally, register the function for undo/redoing
                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::PlaceNotes(chopped_ids, None, curr_track),
                    EditorAction::LengthChange(note_ids, changed_lengths, curr_track),
                ]));
            }
        }
    }
}

// the dialogues for these functions
pub struct EFStretchDialog {
    pub stretch_factor: NumericField<f32>,
    pub is_shown: bool,

    note_editing: Arc<Mutex<NoteEditing>>,
    edit_functions: Rc<RefCell<EditFunctions>>,
    edit_actions: Rc<RefCell<EditorActions>>,
} 

impl Default for EFStretchDialog {
    fn default() -> Self {
        Self { note_editing: Default::default(), edit_functions: Default::default(), edit_actions: Default::default(), stretch_factor: NumericField::new(1.0, Some(0.0), None), is_shown: false }
    }
}

impl EFStretchDialog {
    pub fn new(
        note_editing: &Arc<Mutex<NoteEditing>>,
        edit_functions: &Rc<RefCell<EditFunctions>>,
        edit_actions: &Rc<RefCell<EditorActions>>,
    ) -> Self {
        Self {
            stretch_factor: NumericField::new(1.0, Some(0.0), None),
            is_shown: false,
            note_editing: note_editing.clone(),
            edit_functions: edit_functions.clone(),
            edit_actions: edit_actions.clone()
        }
    }
}

impl Dialog for EFStretchDialog {
    fn show(&mut self) -> () {
        self.is_shown = true;
    }

    fn close(&mut self) -> () {
        self.is_shown = false;
    }

    fn is_showing(&self) -> bool {
        self.is_shown
    }

    fn draw(&mut self, ctx: &egui::Context, _: &ImageResources) -> () {
        if !self.is_showing() { return; }
        egui::Window::new(RichText::new("Stretch Selection").size(15.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                self.stretch_factor.show("Stretch factor (x):", ui, None);
                
                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() {
                        /*if let Some((curr_track, curr_channel)) = parent.get_current_track_and_channel() {
                            let note_editing = parent.note_editing.lock().unwrap();
                            let notes = note_editing.get_notes();
                            let mut notes = notes.write().unwrap();
                            let notes = &mut notes[curr_track as usize];

                            let sel_notes = note_editing.get_selected_note_ids();
                            let mut sel_notes = sel_notes.lock().unwrap();
                            let sel_notes_copy = sel_notes.clone();

                            let mut editor_actions = parent.editor_actions.lock().unwrap();
                            parent.editor_functions.apply_function(notes, &mut sel_notes, EditFunction::Stretch(sel_notes_copy, self.stretch_factor.value() as f32), curr_track, curr_channel, &mut editor_actions);
                        }*/
                        {
                            let note_editing = self.note_editing.lock().unwrap();

                            let notes = note_editing.get_notes();
                            let mut notes = notes.write().unwrap();

                            let sel_notes = note_editing.get_selected_note_ids();
                            let mut sel_notes = sel_notes.lock().unwrap();
                            let sel_notes_copy = sel_notes.clone();

                            let curr_track = note_editing.get_current_track();
                            let notes = &mut notes[curr_track as usize];

                            let mut editor_actions = self.edit_actions.borrow_mut();
                            self.edit_functions.borrow_mut().apply_function(notes, &mut sel_notes, EditFunction::Stretch(sel_notes_copy, self.stretch_factor.value() as f32), curr_track, &mut editor_actions);
                        }
                        self.close();
                    }

                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                })
            });
    }
}

pub struct EFChopDialog {
    pub target_tick_len: NumericField<MIDITick>,
    pub is_shown: bool,

    note_editing: Arc<Mutex<NoteEditing>>,
    edit_functions: Rc<RefCell<EditFunctions>>,
    edit_actions: Rc<RefCell<EditorActions>>,
}

impl Default for EFChopDialog {
    fn default() -> Self {
        Self { note_editing: Default::default(), edit_functions: Default::default(), edit_actions: Default::default(), target_tick_len: NumericField::new(240, Some(1), Some(MIDITick::MAX.into())), is_shown: false }
    }
}

impl EFChopDialog {
    pub fn new(
        note_editing: &Arc<Mutex<NoteEditing>>,
        edit_functions: &Rc<RefCell<EditFunctions>>,
        edit_actions: &Rc<RefCell<EditorActions>>,
    ) -> Self {
        Self {
            target_tick_len: NumericField::new(240, Some(1), Some(MIDITick::MAX.into())),
            is_shown: false,

            note_editing: note_editing.clone(),
            edit_functions: edit_functions.clone(),
            edit_actions: edit_actions.clone()
        }
    }
}

impl Dialog for EFChopDialog {
    fn show(&mut self) -> () {
        self.is_shown = true;
    }

    fn close(&mut self) -> () {
        self.is_shown = false;
    }

    fn is_showing(&self) -> bool {
        self.is_shown
    }

    fn draw(&mut self, ctx: &egui::Context, _: &ImageResources) -> () {
        if !self.is_showing() { return; }
        egui::Window::new(RichText::new("Chop Selection").size(15.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                self.target_tick_len.show("Chop tick length", ui, None);

                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() {
                        {
                            let note_editing = self.note_editing.lock().unwrap();

                            let notes = note_editing.get_notes();
                            let mut notes = notes.write().unwrap();

                            let sel_notes = note_editing.get_selected_note_ids();
                            let mut sel_notes = sel_notes.lock().unwrap();
                            let sel_notes_copy = sel_notes.clone();

                            let curr_track = note_editing.get_current_track();
                            let notes = &mut notes[curr_track as usize];

                            let mut editor_actions = self.edit_actions.borrow_mut();
                            self.edit_functions.borrow_mut().apply_function(notes, &mut sel_notes, EditFunction::Chop(sel_notes_copy, self.target_tick_len.value()), curr_track, &mut editor_actions);
                        }
                        self.close();
                    }

                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                })
            });
    }
}

/*impl EFStretchDialog {
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
}*/