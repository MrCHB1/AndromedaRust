use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}};

use eframe::{
    egui::{self, RichText},
};

use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::Dialog, util::image_loader::ImageResources}, deprecated, editor::{actions::{EditorAction, EditorActions}, editing::note_editing::note_sequence_funcs::{extract, merge_notes, merge_notes_and_return_ids}, util::{bin_search_notes, get_min_max_keys_in_selection, get_min_max_ticks_in_selection, manipulate_note_lengths, manipulate_note_ticks, MIDITick, SignedMIDITick}}, midi::events::note::Note};
use crate::editor::editing::note_editing::NoteEditing;

// modular edit_function
pub enum EditFunction {
    FlipX(Vec<usize>),
    FlipY(Vec<usize>), // key-wise. the easiest to manipulate lol
    Stretch(Vec<usize>, f32),
    //               max
    //               tick len
    Chop(Vec<usize>, MIDITick),
    SliceAtTick(Vec<usize>, MIDITick),
    FadeNotes(bool)
}

#[derive(Default)]
pub struct EditFunctions {

}

impl EditFunctions {
    pub fn apply_function(&mut self, notes: &mut Vec<Note>, sel_note_ids: &mut Vec<usize>, func: EditFunction, curr_track: u16, editor_actions: &mut EditorActions) {
        match func {
            EditFunction::FlipX(_) => {
                deprecated!("use flip x from plugins instead");
            },
            EditFunction::FlipY(_) => {
                deprecated!("use flip y from plugins instead");
            },
            EditFunction::Stretch(note_ids, factor) => {
                if factor == 1.0 { return; }

                let old_notes = std::mem::take(notes);
                let (mut notes_to_stretch, remaining_notes) = extract(old_notes, &note_ids);
                let first_tick = notes_to_stretch[0].start();
                
                let (pos_change, length_change): (Vec<_>, Vec<_>) = notes_to_stretch.iter_mut().map(|note| {
                    let old_length = note.length();
                    let old_tick = note.start();
                    
                    let new_length = (old_length as f32 * factor).round() as MIDITick;
                    let new_tick = ((old_tick - first_tick) as f32 * factor).round() as MIDITick + first_tick;

                    *(note.start_mut()) = new_tick;
                    *(note.length_mut()) = new_length;

                    ((new_tick as SignedMIDITick - old_tick as SignedMIDITick, 0i16), new_length as SignedMIDITick - old_length as SignedMIDITick)
                }).collect();

                let (merged, new_ids) = merge_notes_and_return_ids(remaining_notes, notes_to_stretch);
                *notes = merged;

                editor_actions.register_action(EditorAction::Bulk(vec![
                    // EditorAction::NotesMove(old_ids, new_ids, changed_positions, curr_track, true),
                    EditorAction::LengthChange(note_ids, length_change, curr_track),
                    EditorAction::NotesMove(new_ids, pos_change, curr_track, true),
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

                let old_notes = std::mem::take(notes);
                let (merged, chopped_ids) = merge_notes_and_return_ids(old_notes, new_notes);
                *notes = merged;

                // println!("{:?}", chopped_ids);

                // finally, register the function for undo/redoing
                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::PlaceNotes(chopped_ids, None, curr_track),
                    EditorAction::LengthChange(note_ids, changed_lengths, curr_track),
                ]));
            },
            EditFunction::SliceAtTick(note_ids, slice_tick) => {
                let mut changed_lengths = Vec::with_capacity(sel_note_ids.len());
                let mut new_notes = Vec::with_capacity(sel_note_ids.len());
                
                let mut affected_ids = Vec::with_capacity(note_ids.len());
                for id in note_ids.into_iter() {
                    let note = &mut notes[id];
                    let start = note.start();
                    let end = note.end();

                    // check if the slice tick is between the note, and if it is, make new notes for merging
                    if start < slice_tick && end > slice_tick {
                        let left_len = slice_tick - start;
                        let right_len = end - slice_tick;
                        if left_len == 0 || right_len == 0 { continue; }
                        
                        let old_length = note.length();
                        let new_length = left_len;
                        *(note.length_mut()) = new_length;
                        changed_lengths.push(new_length as SignedMIDITick - old_length as SignedMIDITick);

                        new_notes.push(Note {
                            start: slice_tick,
                            length: right_len,
                            channel: note.channel(),
                            key: note.key(),
                            velocity: note.velocity()
                        });
                        affected_ids.push(id);
                    }
                }

                let old_notes = std::mem::take(notes);
                let (new_notes, new_ids) = merge_notes_and_return_ids(old_notes, new_notes);
                *notes = new_notes;

                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::PlaceNotes(new_ids, None, curr_track),
                    EditorAction::LengthChange(affected_ids, changed_lengths, curr_track)
                ]));
            },
            EditFunction::FadeNotes(fade_out) => {
                let (min_tick, max_tick) = get_min_max_ticks_in_selection(notes, &sel_note_ids).unwrap();
                let mut vel_changes = Vec::with_capacity(sel_note_ids.len());
                for &id in sel_note_ids.iter() {
                    let note = &mut notes[id];

                    let mut vel_fac = (note.start() - min_tick) as f32 / (max_tick as f32 - min_tick as f32);
                    if fade_out { vel_fac = 1.0 - vel_fac; }
                    
                    let old_velocity = note.velocity();
                    let new_velocity = ((old_velocity as f32 * vel_fac) as u8).clamp(1, 127);
                    let vel_change = new_velocity as i8 - old_velocity as i8;
                    vel_changes.push(vel_change);

                    *(note.velocity_mut()) = new_velocity;
                }

                editor_actions.register_action(EditorAction::VelocityChange(sel_note_ids.clone(), vel_changes, curr_track));
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

                            let sel_notes = note_editing.get_shared_selected_ids();
                            let mut sel_notes = sel_notes.write().unwrap();

                            let curr_track = note_editing.get_current_track();
                            let notes = &mut notes[curr_track as usize];

                            let mut sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                            let sel_notes_copy = sel_notes.clone();

                            let mut editor_actions = self.edit_actions.try_borrow_mut().unwrap();
                            self.edit_functions.try_borrow_mut().unwrap().apply_function(notes, &mut sel_notes, EditFunction::Stretch(sel_notes_copy, self.stretch_factor.value() as f32), curr_track, &mut editor_actions);
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

                            let sel_notes = note_editing.get_shared_selected_ids();
                            let mut sel_notes = sel_notes.write().unwrap();

                            let curr_track = note_editing.get_current_track();
                            let notes = &mut notes[curr_track as usize];

                            let mut sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                            let sel_notes_copy = sel_notes.clone();

                            let mut editor_actions = self.edit_actions.try_borrow_mut().unwrap();
                            self.edit_functions.try_borrow_mut().unwrap().apply_function(notes, &mut sel_notes, EditFunction::Chop(sel_notes_copy, self.target_tick_len.value()), curr_track, &mut editor_actions);
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