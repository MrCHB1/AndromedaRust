use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::{Arc, Mutex}};
use eframe::egui;
use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::{Dialog, DialogAction, DialogActionButtons, names::*}, util::image_loader::ImageResources}, deprecated, editor::{actions::{EditorAction, EditorActions}, editing::note_editing::note_sequence_funcs::{extract, merge_notes, merge_notes_and_return_ids}, util::{MIDITick, SignedMIDITick, bin_search_notes, get_min_max_keys_in_selection, get_min_max_ticks_in_selection, manipulate_note_lengths, manipulate_note_ticks}}, midi::events::note::Note};
use crate::editor::editing::note_editing::NoteEditing;

// modular edit_function
pub enum EditFunction {
    FlipX(Vec<usize>),
    FlipY(Vec<usize>), // key-wise. the easiest to manipulate lol
    Stretch(Vec<usize>, f32),
    //               max
    //               tick len
    Chop(Vec<usize>, MIDITick),
    //               glue     |keep channels
    //               threshold|separate
    Glue(Vec<usize>, MIDITick, bool),
    RemoveOverlaps,
    SliceAtTick(Vec<usize>, MIDITick),
    FadeNotes(bool)
}

#[derive(Default)]
pub struct EditFunctions;

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
                // finally, register the function for undo/redoing
                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::PlaceNotes(chopped_ids, None, curr_track),
                    EditorAction::LengthChange(note_ids, changed_lengths, curr_track),
                ]));
            },
            EditFunction::Glue(note_ids, glue_threshold, separate_channels) => {
                if note_ids.is_empty() { return; }

                // take notes out so we can move them without cloning
                let old_notes = std::mem::take(notes);
                let (notes_to_glue, remaining_notes) = extract(old_notes, &note_ids);

                // Table size:
                // - if separate_channels: channel (0..15) * 128 + key (0..127)
                // - if not separate: just key (0..127)
                let table_size = if separate_channels { 16 * 128 } else { 128 };
                
                // table entry: Option<(end_tick, kept_index_in_kept_notes)>
                let mut table: Vec<Option<(MIDITick, usize)>> = vec![None; table_size];

                let mut kept_notes: Vec<Note> = Vec::with_capacity(notes_to_glue.len());
                let mut kept_original_ids: Vec<usize> = Vec::new(); // original selected indices of kept notes (in same order)
                let mut kept_length_changes: Vec<SignedMIDITick> = Vec::new(); // aligned with kept_original_ids
                let mut removed_notes: Vec<Note> = Vec::new(); // notes that were removed (moved out)
                let mut removed_original_ids: Vec<usize> = Vec::new(); // their original indices (for undo if needed)

                for (orig_id, note) in note_ids.into_iter().zip(notes_to_glue.into_iter()) {
                    let key = note.key() as usize;
                    let ch_index = if separate_channels { (note.channel() as usize) * 128 } else { 0 };
                    let table_idx = ch_index + key;

                    let note_start = note.start();
                    let note_end = note.end();

                    match table[table_idx] {
                        None => {
                            let kept_index = kept_notes.len();
                            table[table_idx] = Some((note_end, kept_index));
                            kept_original_ids.push(orig_id);
                            kept_length_changes.push(0); // will be updated if later glued-to
                            kept_notes.push(note);
                        }
                        Some((last_end, kept_index)) => {
                            if note_start <= last_end + glue_threshold {
                                let last_note = &mut kept_notes[kept_index];
                                let old_length = last_note.length();
                                let last_start = last_note.start();
                                let new_end = std::cmp::max(last_end, note_end);
                                let new_length = new_end - last_start;

                                // mutate kept note's length (moved note, no clone)
                                *(last_note.length_mut()) = new_length;

                                // update table's end for future glues on same key/channel
                                table[table_idx] = Some((new_end, kept_index));

                                // record the length delta for this kept note (overwrite if multiple glued notes extend it further)
                                kept_length_changes[kept_index] = new_length as SignedMIDITick - old_length as SignedMIDITick;

                                // record that this note was removed (so undo can re-place it)
                                removed_original_ids.push(orig_id);
                                removed_notes.push(note);
                            } else {
                                // cannot glue: become a new kept note
                                let kept_index = kept_notes.len();
                                table[table_idx] = Some((note_end, kept_index));
                                kept_original_ids.push(orig_id);
                                kept_length_changes.push(0);
                                kept_notes.push(note);
                            }
                        }
                    }
                }

                // merge kept notes back with remaining notes (kept_notes still in ascending start order)
                let (merged, new_ids) = merge_notes_and_return_ids(remaining_notes, kept_notes);
                *notes = merged;

                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::DeleteNotes(removed_original_ids, Some(removed_notes), curr_track),
                    EditorAction::LengthChange(new_ids, kept_length_changes, curr_track),
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
            },
            EditFunction::RemoveOverlaps => {
                if sel_note_ids.is_empty() { return; }

                let old_notes = std::mem::take(notes);
                let (mut extracted_notes, new_notes) = extract(old_notes, sel_note_ids);
                let mut lookup: HashSet<(MIDITick, u8)> = HashSet::with_capacity(notes.len());

                let mut kept_notes: Vec<Note> = Vec::with_capacity(notes.len());
                let mut removed_indices: Vec<usize> = Vec::new();
                let mut removed_notes: Vec<Note> = Vec::new();

                for (idx, note) in extracted_notes.drain(..).enumerate() {
                    let entry = (note.start(), note.key());

                    if lookup.contains(&entry) { // note already exists lol
                        removed_notes.push(note);
                        removed_indices.push(idx);
                    } else {
                        lookup.insert(entry);
                        kept_notes.push(note);
                    }
                }

                let new_notes = merge_notes(new_notes, kept_notes);
                *notes = new_notes;

                if removed_notes.is_empty() { 
                    println!("No overlapped notes were removed.");
                    return;
                }

                println!("Removed {} notes.", removed_indices.len());

                editor_actions.register_action(EditorAction::DeleteNotes(
                    removed_indices, 
                    Some(removed_notes), 
                    curr_track
                ));
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
    fn draw(&mut self, ui: &mut egui::Ui, _: &ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        self.stretch_factor.show("Stretch factor (x):", ui, None);
        None
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(|dlg| {
                let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();
                let dlg_name = dlg.get_dialog_name();

                let note_editing = dlg.note_editing.lock().unwrap();

                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();

                let sel_notes = note_editing.get_shared_selected_ids();
                let mut sel_notes = sel_notes.write().unwrap();

                let curr_track = note_editing.get_current_track();
                let notes = (*tracks)[curr_track as usize].get_notes_mut();

                let mut sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                let sel_notes_copy = sel_notes.clone();

                let mut editor_actions = dlg.edit_actions.try_borrow_mut().unwrap();
                dlg.edit_functions.try_borrow_mut().unwrap().apply_function(notes, &mut sel_notes, EditFunction::Stretch(sel_notes_copy, dlg.stretch_factor.value() as f32), curr_track, &mut editor_actions);
            
                Some(DialogAction::Close(dlg_name))
            }))
        )
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EF_STRETCH
    }

    fn get_dialog_title(&self) -> String {
        "Stretch Selection".into()
    }
    /*fn show(&mut self) -> () {
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
    }*/
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
    fn draw(&mut self, ui: &mut egui::Ui, _: &ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        self.target_tick_len.show("Chop tick length", ui, None);
        None
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(|dlg| {
                let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();
                let dlg_name = dlg.get_dialog_name();

                let note_editing = dlg.note_editing.lock().unwrap();

                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();

                let sel_notes = note_editing.get_shared_selected_ids();
                let mut sel_notes = sel_notes.write().unwrap();

                let curr_track = note_editing.get_current_track();
                let notes = (*tracks)[curr_track as usize].get_notes_mut();

                let mut sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                let sel_notes_copy = sel_notes.clone();

                let mut editor_actions = dlg.edit_actions.try_borrow_mut().unwrap();
                dlg.edit_functions.try_borrow_mut().unwrap().apply_function(notes, &mut sel_notes, EditFunction::Chop(sel_notes_copy, dlg.target_tick_len.value()), curr_track, &mut editor_actions);
            
                Some(DialogAction::Close(dlg_name))
            }))
        )
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EF_STRETCH
    }

    fn get_dialog_title(&self) -> String {
        "Chop selection".into()
    }
}

pub struct EFGlueDialog {
    pub glue_threshold: NumericField<MIDITick>,
    pub separate_channels: bool,
    pub is_shown: bool,

    note_editing: Arc<Mutex<NoteEditing>>,
    edit_functions: Rc<RefCell<EditFunctions>>,
    edit_actions: Rc<RefCell<EditorActions>>
}

impl Default for EFGlueDialog {
    fn default() -> Self {
        Self {
            note_editing: Default::default(),
            edit_functions: Default::default(),
            edit_actions: Default::default(),
            glue_threshold: NumericField::new(0, Some(0), Some(MIDITick::MAX.into())),
            separate_channels: true,
            is_shown: false }
    }
}

impl EFGlueDialog {
    pub fn new(
        note_editing: &Arc<Mutex<NoteEditing>>,
        edit_functions: &Rc<RefCell<EditFunctions>>,
        edit_actions: &Rc<RefCell<EditorActions>>,
    ) -> Self {
        Self {
            glue_threshold: NumericField::new(0, Some(0), Some(MIDITick::MAX.into())),
            separate_channels: true,
            is_shown: false,

            note_editing: note_editing.clone(),
            edit_functions: edit_functions.clone(),
            edit_actions: edit_actions.clone()
        }
    }
}

impl Dialog for EFGlueDialog {
    fn draw(&mut self, ui: &mut egui::Ui, _: &ImageResources) -> Option<DialogAction> {
        self.glue_threshold.show("Glue threshold (in ticks)", ui, None);
        ui.checkbox(&mut self.separate_channels, "Keep channels separate");
        None
    }

    fn get_action_buttons(&self) -> Option<DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(|dlg| {
                let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();
                let dlg_name = dlg.get_dialog_name();

                let note_editing = dlg.note_editing.lock().unwrap();

                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();

                let sel_notes = note_editing.get_shared_selected_ids();
                let mut sel_notes = sel_notes.write().unwrap();

                let curr_track = note_editing.get_current_track();
                let notes = (*tracks)[curr_track as usize].get_notes_mut();

                let mut sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                let sel_notes_copy = sel_notes.clone();

                let mut editor_actions = dlg.edit_actions.try_borrow_mut().unwrap();
                dlg.edit_functions.try_borrow_mut().unwrap().apply_function(notes, &mut sel_notes, EditFunction::Glue(sel_notes_copy, dlg.glue_threshold.value(), dlg.separate_channels), curr_track, &mut editor_actions);
            
                Some(DialogAction::Close(dlg_name))
            }))
        )
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EF_GLUE
    }

    fn get_dialog_title(&self) -> String {
        "Glue notes".into()
    }
}