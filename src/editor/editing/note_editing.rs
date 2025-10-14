#![warn(unused)]
use eframe::egui::{self, Ui};

use crate::{app::{main_window::{EditorTool, EditorToolSettings, ToolBarSettings}, rendering::{data_view::DataViewRenderer, RenderManager, Renderer}}, editor::{actions::{EditorAction, EditorActions}, navigation::PianoRollNavigation, util::{bin_search_notes, find_note_at, get_min_max_ticks_in_selection, get_mouse_midi_pos, get_notes_in_range, MIDITick, SignedMIDITick}}, midi::events::note::Note};
use std::{cell::RefCell, collections::{HashMap, VecDeque}, rc::Rc, sync::{Arc, Mutex, RwLock}};

// flags
pub mod note_flags {
    pub const NOTE_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const NOTE_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const NOTE_EDIT_ANY_DIALOG_OPEN: u16 = 0x2;
    // pub const NOTE_EDIT_TRACK_VIEW: u16 = 0x4;
    pub const NOTE_EDIT_MOUSE_OVER_NOTE: u16 = 0x8;
    pub const NOTE_EDIT_IS_EDITING: u16 = 0x10;
    pub const NOTE_EDIT_MULTIEDIT: u16 = 0x20;
    pub const NOTE_EDIT_DRAGGING: u16 = 0x40;
    pub const NOTE_EDIT_LENGTH_CHANGE: u16 = 0x80;
    pub const NOTE_EDIT_ERASING: u16 = 0x100;

    pub const NOTE_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x1000;
    pub const NOTE_EDIT_SHIFT_DOWN: u16 = 0x2000;
}

use note_flags::*;

pub struct GhostNote {
    id: Option<usize>, // None if the ghost note does not refer to any specific note in the editor
    note: Note
}

impl GhostNote {
    #[inline(always)]
    pub fn get_note_mut(&mut self) -> &mut Note {
        &mut self.note
    }

    #[inline(always)]
    pub fn get_note(&self) -> &Note {
        &self.note
    }
}

impl Default for GhostNote {
    fn default() -> Self {
        Self {
            id: None,
            note: Default::default(),
        }
    }
}

enum NoteSelectionType {
    NewSelect,
    UnionSelect,
    Deselect
}

#[derive(Default)]
pub struct NoteEditing {
    editor_tool: Rc<RefCell<EditorToolSettings>>,
    render_manager: Arc<Mutex<RenderManager>>,
    data_view_renderer: Option<Arc<Mutex<DataViewRenderer>>>,
    notes: Arc<RwLock<Vec<Vec<Note>>>>,
    ghost_notes: Arc<Mutex<Vec<GhostNote>>>,
    selected_notes_ids: Arc<Mutex<Vec<usize>>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    toolbar_settings: Rc<RefCell<ToolBarSettings>>,
    ppq: u16,

    nav: Arc<Mutex<PianoRollNavigation>>,

    // some stuff
    curr_mouse_over_note_idx: Option<usize>,
    curr_mouse_midi_pos: (MIDITick, u8),
    curr_mouse_midi_pos_rounded: (MIDITick, u8),
    is_at_note_end: bool,
    flags: u16,

    // other stuff
    note_old_positions: Vec<(MIDITick, u8)>,
    note_temp_mod_ids: Vec<usize>,
    note_old_lengths: Vec<MIDITick>,
    drag_offset: SignedMIDITick,

    // start tick, end tick, start key, end key
    selection_range: (MIDITick, MIDITick, u8, u8),
    draw_select_box: bool,

    last_clicked_note_idx: usize,

    pub latest_note_start: MIDITick, // for slider scrolling
}

impl NoteEditing {
    pub fn new(
        notes: &Arc<RwLock<Vec<Vec<Note>>>>,
        nav: &Arc<Mutex<PianoRollNavigation>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        render_manager: &Arc<Mutex<RenderManager>>,
        data_view_renderer: &Arc<Mutex<DataViewRenderer>>,
        editor_actions: &Rc<RefCell<EditorActions>>,
        toolbar_settings: &Rc<RefCell<ToolBarSettings>>,
    ) -> Self {
        Self {
            editor_tool: editor_tool.clone(),
            render_manager: render_manager.clone(),
            data_view_renderer: Some(data_view_renderer.clone()),
            notes: notes.clone(),
            ghost_notes: Arc::new(Mutex::new(Vec::new())),
            selected_notes_ids: Arc::new(Mutex::new(Vec::new())),
            editor_actions: editor_actions.clone(),
            toolbar_settings: toolbar_settings.clone(),
            ppq: 960,

            nav: nav.clone(),
            curr_mouse_over_note_idx: None,

            curr_mouse_midi_pos: (0, 0),
            curr_mouse_midi_pos_rounded: (0, 0),
            is_at_note_end: false,
            flags: NOTE_EDIT_FLAGS_NONE,

            note_old_positions: Vec::new(),
            note_temp_mod_ids: Vec::new(),
            note_old_lengths: Vec::new(),
            drag_offset: 0,
            last_clicked_note_idx: 0,
            selection_range: (0, 0, 0, 0),
            draw_select_box: false,

            latest_note_start: 38400,
        }
    }

    #[inline(always)]
    pub fn get_notes(&self) -> Arc<RwLock<Vec<Vec<Note>>>> {
        self.notes.clone()
    }

    #[inline(always)]
    pub fn is_any_note_selected(&self) -> bool {
        let selected = self.selected_notes_ids.lock().unwrap();
        !selected.is_empty()
    }

    #[inline(always)]
    pub fn get_selected_note_ids(&self) -> Arc<Mutex<Vec<usize>>> {
        self.selected_notes_ids.clone()
    }

    /// Moves a note given an index to the [`ghost_notes`] vector.
    /// This also clears the ghost notes.
    fn move_note_ids_to_ghost_note(&mut self, ids: &Vec<usize>, curr_track: u16) {
        if ids.is_empty() { return; }

        let mut notes = self.notes.write().unwrap();
        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        ghost_notes.clear();

        // get notes in current track and channel
        let notes = &mut notes[curr_track as usize];

        {
            let mut rem_offset = 0;
            for id in ids.iter() {
                let real_id = *id - rem_offset;
                let note = notes.remove(real_id);
                let ghost_note = GhostNote {
                    id: Some(real_id),
                    note
                };
                ghost_notes.push(ghost_note);

                // increment to prevent index invalidation
                rem_offset += 1;
            }
        }
    }

    fn move_selected_ids_to_ghost_note(&mut self, curr_track: u16) {
        let sel = self.selected_notes_ids.lock().unwrap();
        if sel.is_empty() { println!("No selected notes to make as ghost notes"); return; }

        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[curr_track as usize];

        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        ghost_notes.clear();

        let mut rem_offset = 0;
        for id in sel.iter() {
            let real_id = *id - rem_offset;

            let note = notes.remove(real_id);
            let ghost_note = GhostNote {
                id: Some(*id),
                note
            };
            ghost_notes.push(ghost_note);

            rem_offset += 1;
        }
    }

    pub fn midi_pos_to_ui_pos(&self, ui: &mut Ui, tick_pos: MIDITick, key_pos: u8) -> (f32, f32) {
        let nav = self.nav.lock().unwrap();
        let rect = ui.min_rect();
        let mut ui_x = (tick_pos as f32 - nav.tick_pos_smoothed) / nav.zoom_ticks_smoothed;
        let mut ui_y = (key_pos as f32 - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;

        ui_x = ui_x * rect.width() + rect.left();
        ui_y = (1.0 - ui_y) * rect.height() + rect.top();

        (ui_x, ui_y)
    }

    /// Will update itself and change some data, mostly from mouse position. Called on every frame.
    pub fn update_from_ui(&mut self, ui: &mut Ui) {
        let (mouse_midi_pos, mouse_midi_pos_rounded) = get_mouse_midi_pos(ui, &self.nav);

        let curr_track = self.get_current_track();
        self.curr_mouse_midi_pos = mouse_midi_pos;
        self.curr_mouse_midi_pos_rounded = mouse_midi_pos_rounded;

        // lets see if we are over a note
        // ...but only if we aren't over the ui
        // first, reset the flag for the mouse being over any note
        self.flags &= !NOTE_EDIT_MOUSE_OVER_NOTE;
        if self.flags & NOTE_EDIT_MOUSE_OVER_UI == 0 {
            let curr_mouse_over_note_idx = {
                let notes = self.notes.read().unwrap();
                let notes = &notes[curr_track as usize];

                find_note_at(notes, mouse_midi_pos.0, mouse_midi_pos.1)
            };

            self.curr_mouse_over_note_idx = curr_mouse_over_note_idx;
            
            // we are over a note!
            if curr_mouse_over_note_idx.is_some() {
                self.flags |= NOTE_EDIT_MOUSE_OVER_NOTE;

                // now check if the mouse is at the note's end (for length changing)
                let nav = self.nav.lock().unwrap();
                self.is_at_note_end = {
                    let notes = self.notes.read().unwrap();
                    let notes = &notes[curr_track as usize];

                    let note = &notes[curr_mouse_over_note_idx.unwrap()];
                    let dist = (note.end() as f32 - mouse_midi_pos.0 as f32) / nav.zoom_ticks_smoothed * 960.0;
                    dist < 2.0
                };

            } else {
                self.is_at_note_end = false;
            }
        } else {
            self.curr_mouse_over_note_idx = None;
        }
    }

    pub fn get_current_track(&self) -> u16 {
        let nav = self.nav.lock().unwrap();
        nav.curr_track
    }

    // flag helper functions
    #[inline(always)]
    pub fn set_mouse_over_ui(&mut self, mouse_over_ui: bool) {
        self.flags &= !NOTE_EDIT_MOUSE_OVER_UI;
        if mouse_over_ui { self.flags |= NOTE_EDIT_MOUSE_OVER_UI; }
    }

    /*#[inline(always)]
    pub fn set_any_dialogs_open(&mut self, any_dialogs_open: bool) {
        self.flags &= !NOTE_EDIT_ANY_DIALOG_OPEN;
        if any_dialogs_open { self.flags |= NOTE_EDIT_ANY_DIALOG_OPEN; }
    }*/

    // the actual editing
    pub fn on_mouse_down(&mut self) {
        if self.flags & NOTE_EDIT_MOUSE_OVER_UI != 0 { 
            self.flags |= NOTE_EDIT_MOUSE_DOWN_ON_UI;
            return;
        }

        if self.flags & NOTE_EDIT_ANY_DIALOG_OPEN != 0 {
            return;
        }

        let curr_track = self.get_current_track();

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {
                self.flags &= !NOTE_EDIT_IS_EDITING;
                self.drag_offset = 0;

                if let Some(clicked_note_idx) = self.curr_mouse_over_note_idx {
                    // we are over a note!
                    
                    // check selected notes and determine if we should modify multiple notes
                    self.flags &= !NOTE_EDIT_MULTIEDIT;
                    {
                        let mut sel = self.selected_notes_ids.lock().unwrap();
                        if !sel.is_empty() {
                            if sel.contains(&clicked_note_idx) && sel.len() > 1 {
                                self.flags |= NOTE_EDIT_MULTIEDIT;
                            } else {
                                // we aren't over any selected note
                                sel.clear();
                            }
                        }
                    }

                    // update toolbar settings to match the clicked note
                    {
                        let curr_track = self.get_current_track();
                        let notes = self.notes.read().unwrap();
                        let note = &notes[curr_track as usize][clicked_note_idx];

                        let mut tbs = self.toolbar_settings.borrow_mut();
                        tbs.note_gate.set_value(note.length());
                        tbs.note_velocity.set_value(note.velocity());
                        tbs.note_channel.set_value(note.channel() + 1);
                    }

                    self.flags &= !NOTE_EDIT_LENGTH_CHANGE;
                    self.flags &= !NOTE_EDIT_DRAGGING;

                    // edit multiple notes
                    if self.flags & NOTE_EDIT_MULTIEDIT != 0 {
                        // setup selected notes for dragging
                        if self.is_at_note_end {
                            self.setup_notes_for_length_change(None, curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_LENGTH_CHANGE;
                        } else {
                            self.setup_notes_for_drag(None, curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_DRAGGING;
                        }
                    } else {
                        if self.is_at_note_end {
                            self.setup_notes_for_length_change(Some(vec![clicked_note_idx]), curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_LENGTH_CHANGE;
                        } else {
                            self.setup_notes_for_drag(Some(vec![clicked_note_idx]), curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_DRAGGING;
                        }
                    }
                    self.last_clicked_note_idx = clicked_note_idx;

                // we are not over a note
                } else {
                    // clear selected notes because we are not over any note at all
                    {
                        let mut sel = self.selected_notes_ids.lock().unwrap();
                        sel.clear();
                    }

                    // make a new ghost note at index zero... if it is empty
                    // otherwise, just go ahead n modify the one at index 0
                    {
                        self.update_first_ghost_note();
                        /*let mut ghost_notes = self.ghost_notes.lock().unwrap();
                        if ghost_notes.is_empty() {
                            let tbs = self.toolbar_settings.lock().unwrap();
                            
                        }*/
                        // if ghost_notes.is_empty() { ghost_notes.push(Default::default()); }
                        self.flags |= NOTE_EDIT_IS_EDITING;
                    }
                }

                // show ghost notes if we are not dragging
                if self.flags & NOTE_EDIT_LENGTH_CHANGE == 0 {
                    self.show_ghost_notes();
                }
            },
            EditorTool::Eraser => {
                if let Some(clicked_note_idx) = self.curr_mouse_over_note_idx {
                    // delete the oote immediately when clicking on it
                    self.delete_notes(vec![clicked_note_idx], curr_track);
                } else {
                    self.flags |= NOTE_EDIT_ERASING;
                    self.init_selection_box(self.curr_mouse_midi_pos.0, self.curr_mouse_midi_pos_rounded.1);
                }
            },
            EditorTool::Selector => {
                self.flags &= !NOTE_EDIT_DRAGGING;
                self.flags &= !NOTE_EDIT_LENGTH_CHANGE;
                self.flags &= !NOTE_EDIT_MULTIEDIT;
                
                if let Some(clicked_note_idx) = self.curr_mouse_over_note_idx {
                    // we are over a note!
                    // if we did click a selected note, set the flag for dragging or length changing

                    let should_modify_selected = {
                        let selected_ids = self.selected_notes_ids.lock().unwrap();
                        !selected_ids.is_empty() && selected_ids.contains(&clicked_note_idx)
                    };

                    if should_modify_selected {
                        self.flags |= NOTE_EDIT_MULTIEDIT;
                        if self.is_at_note_end {
                            self.setup_notes_for_length_change(None, curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_LENGTH_CHANGE;
                        } else {
                            self.setup_notes_for_drag(None, curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_DRAGGING;
                        }
                    } else {
                        if self.is_at_note_end {
                            self.setup_notes_for_length_change(Some(vec![clicked_note_idx]), curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_LENGTH_CHANGE;
                        } else {
                            self.setup_notes_for_drag(Some(vec![clicked_note_idx]), curr_track, clicked_note_idx);
                            self.flags |= NOTE_EDIT_DRAGGING;
                        }
                    }

                    // always guaranteed to show at least one ghost note, unless something got borked lol
                    self.show_ghost_notes();
                } else {
                    self.init_selection_box(self.curr_mouse_midi_pos.0, self.curr_mouse_midi_pos_rounded.1);
                }
            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.flags & NOTE_EDIT_MOUSE_OVER_UI != 0 { 
            self.flags |= NOTE_EDIT_MOUSE_DOWN_ON_UI;
            return;
        }

        if self.flags & NOTE_EDIT_ANY_DIALOG_OPEN != 0 {
            return;
        }

        let curr_track = self.get_current_track();

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil | EditorTool::Eraser => {
                if let Some(clicked_note_idx) = self.curr_mouse_over_note_idx {
                    self.delete_notes(vec![clicked_note_idx], curr_track);
                }
            },
            EditorTool::Selector => {
                
            }
        }
    }

    pub fn on_mouse_move(&mut self) {
        if self.flags & NOTE_EDIT_MOUSE_DOWN_ON_UI != 0 { return; }

        if self.flags & (NOTE_EDIT_MOUSE_OVER_UI | NOTE_EDIT_ANY_DIALOG_OPEN) != 0 { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                // update ghost notes (if there is any)

                if self.flags & NOTE_EDIT_IS_EDITING != 0 {
                    self.update_first_ghost_note();
                    /*let gn_start = {
                        let mut snapped = self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick);
                        if snapped < 0 {
                            snapped = 0;
                        }
                        snapped
                    } as MIDITick;

                    let gn_key = {
                        let mut key = self.curr_mouse_midi_pos.1 as u8;
                        if key > 127 {
                            key = 127;
                        }
                        key
                    };

                    let tbs = self.toolbar_settings.lock().unwrap();
                    let gn_length = tbs.note_gate.value() as MIDITick;
                    let gn_velocity = tbs.note_velocity.value() as u8;
                    let gn_channel = tbs.note_channel.value() as u8 - 1;
                    drop(tbs);
                    
                    if !ghost_notes.is_empty() {
                        let gn = ghost_notes[0].get_note_mut();
                        gn.start = gn_start;
                        gn.key = gn_key;
                        gn.length = gn_length;
                        gn.velocity = gn_velocity;
                        gn.channel = gn_channel;
                    } else {
                        ghost_notes.push(GhostNote { id: None, note: Note { channel: gn_channel, start: gn_start, length: gn_length, key: gn_key, velocity: gn_velocity } });
                    }*/
                } else if self.flags & NOTE_EDIT_DRAGGING != 0 {
                    let mut ghost_notes = self.ghost_notes.lock().unwrap();

                    // single-note editing
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        if !ghost_notes.is_empty() {
                            let gn = ghost_notes[0].get_note_mut();
                            gn.start = {
                                let mut snapped =
                                    self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset as SignedMIDITick);
                                if snapped < 0 {
                                    snapped = 0;
                                }
                                snapped
                            } as MIDITick;
                            gn.key = {
                                let mut key = self.curr_mouse_midi_pos.1 as u8;
                                if key > 127 {
                                    key = 127;
                                }
                                key
                            };
                        }
                    // multi-note editing
                    } else {
                        let (cn_start, cn_key) = {
                            let clicked_note = &ghost_notes[self.note_temp_mod_ids[0]].note;
                            (clicked_note.start, clicked_note.key)
                        };

                        let base_start = {
                            let mut snapped =
                                self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset as SignedMIDITick);
                            if snapped < 0 {
                                snapped = 0;
                            }
                            snapped
                        } as MIDITick;

                        let base_key = self.curr_mouse_midi_pos.1;

                        for ghost_note in ghost_notes.iter_mut() {
                            // use temp_note_positions for calculating the offset from the clicked note index - so all ghost notes don't end up on the same position
                            // drag
                            let (tick_d, key_d) = {
                                let tick_d = ghost_note.note.start as SignedMIDITick - cn_start as SignedMIDITick;
                                let key_d = ghost_note.note.key as i16 - cn_key as i16;
                                (tick_d, key_d)
                            };

                            ghost_note.note.start = {
                                let mut new_start = base_start as SignedMIDITick + tick_d;
                                if new_start < 0 {
                                    new_start = 0;
                                }
                                new_start
                            } as MIDITick;

                            ghost_note.note.key = {
                                let mut new_key = base_key as i16 + key_d;
                                if new_key < 0 {
                                    new_key = 0;
                                }
                                if new_key > 127 {
                                    new_key = 127;
                                }
                                new_key
                            } as u8;
                        }
                    }
                } else if self.flags & NOTE_EDIT_LENGTH_CHANGE != 0 {
                    let curr_track = self.get_current_track();
                    
                    let mut notes = self.notes.write().unwrap();
                    let notes = &mut notes[curr_track as usize];
                    for (i, id) in self.note_temp_mod_ids.iter().enumerate() {
                        let old_length = self.note_old_lengths[i];

                        // get the note to change the length of
                        let note = &mut notes[*id];

                        let new_note_end = self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset) + old_length as SignedMIDITick;
                        let mut new_note_length = new_note_end - note.start as SignedMIDITick;
                        
                        let min_possible_length = {
                            let min_snap = self.get_min_snap_tick_length() as SignedMIDITick;
                            let end_modulo = new_note_end % min_snap;
                            if end_modulo == 0 { min_snap }
                            else { end_modulo }
                        };

                        if new_note_length < min_possible_length as SignedMIDITick {
                            new_note_length = min_possible_length;
                        }

                        note.length = new_note_length as MIDITick;
                    }
                }
            },
            EditorTool::Eraser => {
                if self.draw_select_box {
                    self.update_selection_box(self.curr_mouse_midi_pos.0, self.curr_mouse_midi_pos_rounded.1);
                }
            },
            EditorTool::Selector => {
                if self.flags & NOTE_EDIT_DRAGGING != 0 { // we are dragging notes here
                    let mut ghost_notes = self.ghost_notes.lock().unwrap();

                    // single-note editing
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        if !ghost_notes.is_empty() {
                            let gn = ghost_notes[0].get_note_mut();
                            gn.start = {
                                let mut snapped =
                                    self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset as SignedMIDITick);
                                if snapped < 0 {
                                    snapped = 0;
                                }
                                snapped
                            } as MIDITick;
                            gn.key = {
                                let mut key = self.curr_mouse_midi_pos.1 as u8;
                                if key > 127 {
                                    key = 127;
                                }
                                key
                            };
                        }
                    // multi-note editing
                    } else {
                        let (cn_start, cn_key) = {
                            let clicked_note = &ghost_notes[self.note_temp_mod_ids[0]].note;
                            (clicked_note.start, clicked_note.key)
                        };

                        let base_start = {
                            let mut snapped =
                                self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset as SignedMIDITick);
                            if snapped < 0 {
                                snapped = 0;
                            }
                            snapped
                        } as MIDITick;

                        let base_key = self.curr_mouse_midi_pos.1;

                        for ghost_note in ghost_notes.iter_mut() {
                            // use temp_note_positions for calculating the offset from the clicked note index - so all ghost notes don't end up on the same position
                            // drag
                            let (tick_d, key_d) = {
                                let tick_d = ghost_note.note.start as SignedMIDITick - cn_start as SignedMIDITick;
                                let key_d = ghost_note.note.key as i16 - cn_key as i16;
                                (tick_d, key_d)
                            };

                            ghost_note.note.start = {
                                let mut new_start = base_start as SignedMIDITick + tick_d;
                                if new_start < 0 {
                                    new_start = 0;
                                }
                                new_start
                            } as MIDITick;

                            ghost_note.note.key = {
                                let mut new_key = base_key as i16 + key_d;
                                if new_key < 0 {
                                    new_key = 0;
                                }
                                if new_key > 127 {
                                    new_key = 127;
                                }
                                new_key
                            } as u8;
                        }
                    }
                } else if self.flags & NOTE_EDIT_LENGTH_CHANGE != 0 {
                    let curr_track = self.get_current_track();
                    
                    let mut notes = self.notes.write().unwrap();
                    let notes = &mut notes[curr_track as usize];
                    for (i, id) in self.note_temp_mod_ids.iter().enumerate() {
                        let old_length = self.note_old_lengths[i];

                        // get the note to change the length of
                        let note = &mut notes[*id];

                        let new_note_end = self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick + self.drag_offset) + old_length as SignedMIDITick;
                        let mut new_note_length = new_note_end - note.start as SignedMIDITick;
                        
                        let min_possible_length = {
                            let min_snap = self.get_min_snap_tick_length() as SignedMIDITick;
                            let end_modulo = new_note_end % min_snap;
                            if end_modulo == 0 { min_snap }
                            else { end_modulo }
                        };

                        if new_note_length < min_possible_length as SignedMIDITick {
                            new_note_length = min_possible_length;
                        }

                        note.length = new_note_length as MIDITick;
                    }
                } else { 
                    if self.draw_select_box {
                        self.update_selection_box(self.curr_mouse_midi_pos.0, self.curr_mouse_midi_pos_rounded.1);
                    }
                }
            }
        }
    }

    fn finalize_single_note_drag(&mut self) {
        let (old_tick, old_key) = self.note_old_positions.pop().unwrap();
        let (new_tick, new_key) = {
            let ghost_notes = self.ghost_notes.lock().unwrap();
            let ghost_note = ghost_notes[0].get_note();
            (ghost_note.start, ghost_note.key)
        };

        let (tick_d, key_d) = (
            new_tick as SignedMIDITick - old_tick as SignedMIDITick,
            new_key as i16 - old_key as i16,
        );

        self.hide_ghost_notes();
        self.apply_ghost_notes(EditorAction::NotesMove(
            Default::default(),
            Default::default(),
            vec![(tick_d, key_d)],
            Default::default(),
            false
        ));
    }

    fn finalize_multi_note_drag(&mut self) {
        let mut midi_pos_changes = Vec::new();

        {
            let ghost_notes = self.ghost_notes.lock().unwrap();
            for (i, ghost_note) in ghost_notes.iter().enumerate() {
                let (old_tick, old_key) = self.note_old_positions[i];
                let (new_tick, new_key) = {
                    let ghost_note = ghost_note.get_note();
                    (ghost_note.start, ghost_note.key)
                };

                let midi_pos_change = (
                    new_tick as SignedMIDITick - old_tick as SignedMIDITick,
                    new_key as i16 - old_key as i16,
                );

                midi_pos_changes.push(midi_pos_change);
            }

            self.note_old_positions.clear();
        }

        self.hide_ghost_notes();
        self.apply_ghost_notes(EditorAction::NotesMove(
            Default::default(),
            Default::default(),
            midi_pos_changes,
            Default::default(),
            true
        ));
    }

    fn finalize_single_note_length_change(&mut self) {
        let curr_track = self.get_current_track();

        let note_id = self.note_temp_mod_ids.pop().unwrap();
        let old_length = self.note_old_lengths.pop().unwrap();

        // get the note we're changing the length of
        let notes = self.notes.read().unwrap();
        let note = &notes[curr_track as usize][note_id];

        let length_diff = note.length as SignedMIDITick - old_length as SignedMIDITick;

        {
            let mut tbs = self.toolbar_settings.borrow_mut();
            tbs.note_gate.set_value(note.length());
        }

        if length_diff != 0 {
            let mut editor_actions = self.editor_actions.borrow_mut();
            editor_actions.register_action(EditorAction::LengthChange(
                vec![note_id],
                vec![length_diff],
                curr_track
            ));
        }
    }

    fn finalize_multi_note_length_change(&mut self) {
        let curr_track = self.get_current_track();
        
        let notes = self.notes.read().unwrap();
        let notes = &notes[curr_track as usize];

        // to ignore notes that didn't change in length
        let mut length_diffs = Vec::new();
        let mut valid_note_ids = Vec::new();

        for (i, tmp_mod_id) in self.note_temp_mod_ids.iter().enumerate() {
            let id = *tmp_mod_id;
            let old_length = self.note_old_lengths[i];

            let note = &notes[id];

            let length_diff = note.length as SignedMIDITick - old_length as SignedMIDITick;
            if length_diff != 0 {
                length_diffs.push(length_diff);
                valid_note_ids.push(id);
            }
        }

        if !length_diffs.is_empty() {
            let mut editor_actions = self.editor_actions.borrow_mut();
            editor_actions.register_action(EditorAction::LengthChange(
                valid_note_ids,
                length_diffs,
                curr_track
            ));
        }
    }

    pub fn on_mouse_up(&mut self) {
        if self.flags & (NOTE_EDIT_MOUSE_DOWN_ON_UI) != 0 {
            self.flags &= !NOTE_EDIT_MOUSE_DOWN_ON_UI;
            return;
        }

        if self.flags & (NOTE_EDIT_MOUSE_OVER_UI | NOTE_EDIT_ANY_DIALOG_OPEN) != 0 { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                if self.flags & NOTE_EDIT_IS_EDITING != 0 {
                    self.hide_ghost_notes();
                    self.apply_ghost_notes(EditorAction::PlaceNotes(Default::default(), None, Default::default()));
                    self.flags &= !NOTE_EDIT_IS_EDITING;
                } else if self.flags & NOTE_EDIT_DRAGGING != 0 {
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        // single note has been dragged
                        self.finalize_single_note_drag();
                    } else {
                        // multiple notes have been dragged
                        self.finalize_multi_note_drag();
                    }
                    self.flags &= !(NOTE_EDIT_DRAGGING | NOTE_EDIT_MULTIEDIT);
                } else if self.flags & NOTE_EDIT_LENGTH_CHANGE != 0 {
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        // single note has changed length
                        self.finalize_single_note_length_change();
                    } else {
                        // mulitple notes had changed lengths
                        self.finalize_multi_note_length_change();
                    }

                    self.flags &= !(NOTE_EDIT_LENGTH_CHANGE | NOTE_EDIT_MULTIEDIT);
                }

                self.update_latest_note_start();
            },
            EditorTool::Eraser => {
                self.flags &= !NOTE_EDIT_ERASING;
                self.draw_select_box = false;

                let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();
                let curr_track = self.get_current_track();

                let mut selected = {
                    let notes = self.notes.read().unwrap();
                    let notes = &notes[curr_track as usize];

                    get_notes_in_range(notes, min_tick, max_tick, min_key, max_key, true)
                };

                println!("{:?}", selected);

                if !selected.is_empty() {
                    self.delete_notes(std::mem::take(&mut selected), curr_track);
                }
            },
            EditorTool::Selector => {
                if self.flags & NOTE_EDIT_DRAGGING != 0 {
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        // single note has been dragged
                        self.finalize_single_note_drag();
                    } else {
                        // multiple notes have been dragged
                        self.finalize_multi_note_drag();
                    }
                    self.flags &= !(NOTE_EDIT_DRAGGING | NOTE_EDIT_MULTIEDIT);
                } else if self.flags & NOTE_EDIT_LENGTH_CHANGE != 0 {
                    if self.flags & NOTE_EDIT_MULTIEDIT == 0 {
                        // single note has changed length
                        self.finalize_single_note_length_change();
                    } else {
                        // mulitple notes had changed lengths
                        self.finalize_multi_note_length_change();
                    }

                    self.flags &= !(NOTE_EDIT_LENGTH_CHANGE | NOTE_EDIT_MULTIEDIT);
                } else {
                    self.draw_select_box = false;

                    let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();
                    let curr_track = self.get_current_track();

                    let notes = self.notes.read().unwrap();
                    let notes = &notes[curr_track as usize];

                    let selected = get_notes_in_range(notes, min_tick, max_tick, min_key, max_key, true);

                    {
                        let mut sel = self.selected_notes_ids.lock().unwrap();
                        let mut editor_actions = self.editor_actions.borrow_mut();

                        if !selected.is_empty() {
                            editor_actions.register_action(EditorAction::Select(selected.clone(), curr_track));
                        } else if !sel.is_empty() {
                            editor_actions.register_action(EditorAction::Deselect(std::mem::take(&mut sel), curr_track));
                        }

                        *sel = selected;
                    }

                    {
                        let mut render_manger = self.render_manager.lock().unwrap();
                        render_manger.get_active_renderer().lock().unwrap().set_selected(self.selected_notes_ids.clone());
                    }
                }
            }
        }
    }

    pub fn on_key_down(&mut self, ui: &mut Ui, ctrl_down: bool) {
        // undo/redo
        /*if ctrl_down {
            if ui.input(|i| i.key_pressed(egui::Key::Z)) {
                let action = {
                    let mut editor_actions = self.editor_actions.lock().unwrap();
                    editor_actions.undo_action()
                };

                if action.is_some() {
                    self.apply_action(action.unwrap());
                }
            }

            if ui.input(|i| i.key_pressed(egui::Key::Y)) {
                let action = {
                    let mut editor_actions = self.editor_actions.lock().unwrap();
                    editor_actions.redo_action()
                };

                if action.is_some() {
                    self.apply_action(action.unwrap());
                }
            }
        }*/

        // duplicate notes
        if ctrl_down {
            if ui.input(|i| i.key_pressed(egui::Key::D)) {
                let curr_track = self.get_current_track();
                let (sel_notes_dupe, _, max_tick) = {
                    let notes = self.notes.read().unwrap();
                    let notes = &notes[curr_track as usize];
                    let sel_notes = self.selected_notes_ids.lock().unwrap();
                    if let Some((min_tick, max_tick)) = get_min_max_ticks_in_selection(notes, &sel_notes) {
                        (sel_notes.to_vec(), min_tick, max_tick)
                    } else {
                        return;
                    }
                };

                self.duplicate_notes(
                    sel_notes_dupe,
                    max_tick,
                    curr_track, 
                    curr_track,
                    true
                );
            }
        }

        // delete notes
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete_selected_notes();
        }
    }

    pub fn set_key_shift_flag(&mut self, shift_down: bool) {
        self.flags &= !NOTE_EDIT_SHIFT_DOWN;
        if shift_down { self.flags |= NOTE_EDIT_SHIFT_DOWN; }
    }
/* 
    ===== EDITOR HELPER FUNCTIONS =====
*/
    fn delete_notes(&mut self, mut ids: Vec<usize>, curr_track: u16) {
        let mut notes_deleted_deque = VecDeque::with_capacity(ids.len());

        //let mut ids = ids.lock().unwrap();
        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[curr_track as usize];

        let mut applied_ids = std::mem::take(&mut ids);
        applied_ids.sort_by_key(|id| *id);

        for &id in applied_ids.iter().rev() {
            let removed_note = notes.remove(id);
            notes_deleted_deque.push_front(removed_note); // push to front to overall maintain a sorted deleted array
        }

        // self.note_user_deleted.push_front(notes_deleted_deque);
        let mut editor_actions = self.editor_actions.borrow_mut();
        editor_actions.register_action(EditorAction::DeleteNotes(applied_ids, Some(notes_deleted_deque), curr_track));
    }

    pub fn delete_selected_notes(&mut self) {
        let curr_track = self.get_current_track();
        {
            let selected = {
                let mut sel = self.selected_notes_ids.lock().unwrap();
                std::mem::take(&mut *sel)
            };

            self.delete_notes(selected, curr_track);
        }
    }

    /// Makes/adds/removes a selection of notes in [`track`] that correspond to [`ids`].
    /// - [`NoteSelectionType::NewSelect`]: a new selection is made, discarding any old selection.
    /// - [`NoteSelectionType::UnionSelect`]: a new selection is made, without deselecting any notes.
    /// - [`NoteSelectionType::Deselect`]: removes selection until none of [`ids`] is present in the selection.
    fn select_notes(&mut self, track: u16, ids: Vec<usize>, select_type: NoteSelectionType) {
        let mut sel = self.selected_notes_ids.lock().unwrap();
        let mut editor_actions = self.editor_actions.borrow_mut();

        match select_type {
            NoteSelectionType::NewSelect => {
                if !ids.is_empty() { 
                    // editor_actions.register_action(EditorAction::Select(ids.clone(), curr_track as u32 * 16 + curr_channel as u32));
                } else if !sel.is_empty() {
                    editor_actions.register_action(EditorAction::Deselect(std::mem::take(&mut sel), track));
                }
            },
            NoteSelectionType::UnionSelect => {
                todo!("Add new selected notes")
            },
            NoteSelectionType::Deselect => {

            }
        }
    }

    fn update_first_ghost_note(&mut self) {
        let mut ghost_notes = self.ghost_notes.lock().unwrap();

        let gn_start = {
            let mut snapped = self.snap_tick(self.curr_mouse_midi_pos.0 as SignedMIDITick);
            if snapped < 0 {
                snapped = 0;
            }
            snapped
        } as MIDITick;
        
        let gn_key = {
            let mut key = self.curr_mouse_midi_pos.1 as u8;
            if key > 127 {
                key = 127;
            }
            key
        };

        let tbs = self.toolbar_settings.borrow_mut();
        let gn_length = tbs.note_gate.value() as MIDITick;
        let gn_velocity = tbs.note_velocity.value() as u8;
        let gn_channel = tbs.note_channel.value() as u8 - 1;
        drop(tbs);
        
        if !ghost_notes.is_empty() {
            let gn = ghost_notes[0].get_note_mut();
            gn.start = gn_start;
            gn.key = gn_key;
            gn.length = gn_length;
            gn.velocity = gn_velocity;
            gn.channel = gn_channel;
        } else {
            ghost_notes.push(GhostNote { id: None, note: Note { channel: gn_channel, start: gn_start, length: gn_length, key: gn_key, velocity: gn_velocity } });
        }
    }

    /// Returns the IDs of newly duplicated notes. The IDs belong to [`note_group_dst`].
    fn duplicate_notes(&mut self, note_ids: Vec<usize>, paste_tick: MIDITick, track_src: u16, track_dst: u16, select_duplicate: bool) -> Vec<usize> {
        let mut notes = self.notes.write().unwrap();

        // let (src_track, src_channel) = decode_note_group(note_group_src);
        // let (dst_track, dst_channel) = decode_note_group(note_group_dst);

        let (mut notes_src, mut notes_dst) =
            if track_src == track_dst {
                (&mut notes[track_src as usize], None)
            } else {
                let (low, high) = notes.split_at_mut(std::cmp::max(track_src, track_dst) as usize);
                if track_src < track_dst {
                    (&mut low[track_src as usize],
                        Some(&mut high[0]))
                } else {
                    (&mut high[0],
                        Some(&mut low[track_dst as usize]))
                }
            };
            /*if src_track == dst_track && src_channel == dst_channel {
                (&mut notes[src_track as usize], None)
            } else {
                if src_track != dst_track {
                    let (low, high) = notes.split_at_mut(std::cmp::max(src_track, dst_track) as usize);
                    if src_track < dst_track {
                        (&mut low[src_track as usize][src_channel as usize],
                        Some(&mut high[0][dst_channel as usize]))
                    } else {
                        (&mut high[0][src_channel as usize],
                        Some(&mut low[dst_track as usize][dst_channel as usize]))
                    }
                } else {
                    let track_notes = &mut notes[src_track as usize];
                    let (low, high) = track_notes.split_at_mut(std::cmp::max(src_channel, dst_channel) as usize);
                    if src_channel < dst_channel {
                        (&mut low[src_channel as usize],
                        Some(&mut high[0]))
                    } else {
                        (&mut high[0],
                        Some(&mut low[dst_channel as usize]))
                    }
                }
            };*/

        let mut paste_ids = Vec::new();

        {
            // deselect all notes
            let mut sel_notes = self.selected_notes_ids.lock().unwrap();
            sel_notes.clear();
        }

        let mut unique_id_hash = HashMap::new();

        // bruh this is gross
        if notes_dst.is_none() {
            let dst = &mut notes_src;
            let first_tick = dst[note_ids[0]].start;
            for &id in note_ids.iter() {
                let note_copy = {
                    let note = &dst[id];
                    Note {
                        start: note.start - first_tick + paste_tick,
                        length: note.length,
                        key: note.key,
                        velocity: note.velocity,
                        channel: note.channel
                    }
                };

                let insert_idx = bin_search_notes(&dst, note_copy.start);
                let offset = unique_id_hash.entry(insert_idx).or_insert(0);
                let unique_id = insert_idx + *offset;
                paste_ids.push(unique_id);

                if select_duplicate { // select the duplicate notes
                    let mut sel_notes = self.selected_notes_ids.lock().unwrap();
                    sel_notes.push(unique_id);
                }

                dst.insert(insert_idx, note_copy);

                *offset += 1;
            }
        } else {
            let dst = notes_dst.take().unwrap();
            let first_tick = &notes_src[note_ids[0]].start;

            for &id in note_ids.iter() {
                let note_copy = {
                    let note = &notes_src[id];
                    Note {
                        start: note.start - first_tick + paste_tick,
                        length: note.length,
                        key: note.key,
                        velocity: note.velocity,
                        channel: note.channel
                    }
                };

                let insert_idx = bin_search_notes(&dst, note_copy.start);
                let offset = unique_id_hash.entry(insert_idx).or_insert(0);
                let unique_id = insert_idx + *offset;
                paste_ids.push(unique_id);

                if select_duplicate { // select the duplicate notes
                    let mut sel_notes = self.selected_notes_ids.lock().unwrap();
                    sel_notes.push(unique_id);
                }

                dst.insert(insert_idx, note_copy);

                *offset += 1;
            }
        }

        let mut editor_actions = self.editor_actions.borrow_mut();
        editor_actions.register_action(EditorAction::Duplicate(paste_ids.clone(), paste_tick, track_src, track_dst));
        paste_ids
    }

    /// Prepares the editor for changing the length of notes.
    /// if [`note_ids`] is `None`, then the function will use selected notes ids.
    fn setup_notes_for_length_change(&mut self, note_ids: Option<Vec<usize>>, curr_track: u16, base_id: usize) {
        // clear any old length arrays
        self.note_old_lengths.clear();
        self.note_temp_mod_ids.clear();

        let should_use_selected = note_ids.is_none();

        let notes = self.notes.read().unwrap();
        let notes = &notes[curr_track as usize];

        self.drag_offset = {
            let base_note = &notes[base_id];
            base_note.start as SignedMIDITick - (self.curr_mouse_midi_pos_rounded.0 as SignedMIDITick)
        };

        if should_use_selected {
            let sel_ids = self.selected_notes_ids.lock().unwrap();
            for id in sel_ids.iter() {
                let note = &notes[*id];
                self.note_old_lengths.push(note.length);
                self.note_temp_mod_ids.push(*id);
            }
        } else {
            for id in note_ids.unwrap().iter() {
                let note = &notes[*id];
                self.note_old_lengths.push(note.length);
                self.note_temp_mod_ids.push(*id);
            }
        }
    }

    /// Prepares the editor for dragging notes.
    /// if [`note_ids`] is `None`, then the function will use selected notes ids.
    fn setup_notes_for_drag(&mut self, note_ids: Option<Vec<usize>>, curr_track: u16, base_id: usize) {
        // clear any old pos arrays
        self.note_old_positions.clear();
        self.note_temp_mod_ids.clear();

        // set drag offset
        self.drag_offset = {
            let notes = self.notes.read().unwrap();
            let base_note = &notes[curr_track as usize][base_id];
            base_note.start as SignedMIDITick - (self.curr_mouse_midi_pos.0 as SignedMIDITick)
        };
        
        let should_use_selected = note_ids.is_none();

        // move these notes to be ghost notes
        if should_use_selected {
            self.move_selected_ids_to_ghost_note(curr_track);
        } else {
            self.move_note_ids_to_ghost_note(note_ids.as_ref().unwrap(), curr_track);
        }

        //let note_ids = note_ids.as_ref().unwrap();
        let ghost_notes = self.ghost_notes.lock().unwrap();
        // save the old note positions
        for (i, ghost_note) in ghost_notes.iter().enumerate() {
            let note = ghost_note.note;
            self.note_old_positions.push((note.start, note.key));

            // ONLY for dragging multiple notes
            if ghost_note.id.is_none() { continue; }
            if should_use_selected && ghost_note.id.unwrap() == base_id {
                self.note_temp_mod_ids = vec![i];
            }
        }
    }

    fn show_ghost_notes(&mut self) {
        {
            let mut render_manager = self.render_manager.lock().unwrap();
            let curr_renderer = render_manager.get_active_renderer();
            curr_renderer.lock().unwrap().set_ghost_notes(self.ghost_notes.clone());
        }

        {
            let mut data_view_renderer = self.data_view_renderer.as_ref().unwrap().lock().unwrap();
            data_view_renderer.set_ghost_notes(self.ghost_notes.clone());
        }
    }

    fn hide_ghost_notes(&mut self) {
        {
            let mut render_manager = self.render_manager.lock().unwrap();
            let curr_renderer = render_manager.get_active_renderer();
            curr_renderer.lock().unwrap().clear_ghost_notes();
        }

        {
            let mut data_view_renderer = self.data_view_renderer.as_ref().unwrap().lock().unwrap();
            data_view_renderer.clear_ghost_notes();
        }
    }

    fn apply_ghost_notes(&mut self, action: EditorAction) {
        let curr_track = self.get_current_track();
        let mut notes = self.notes.write().unwrap();
        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        let notes = &mut notes[curr_track as usize];

        // to be stored in editor_actions
        let (mut old_ids, mut new_ids): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());

        let is_moving_selected = {
            let sel = self.selected_notes_ids.lock().unwrap();
            sel.len() > 0
        };

        let mut id_compensation: HashMap<usize, usize> = HashMap::new();
        for (i, gnote) in ghost_notes.iter().enumerate() {
            let note = gnote.get_note();
            let insert_idx = bin_search_notes(notes, note.start);
            let offset = id_compensation.entry(insert_idx).or_insert(0);
            let real_idx = insert_idx + *offset;

            old_ids.push(gnote.id.unwrap_or(insert_idx));
            new_ids.push(real_idx);
            (*notes).insert(insert_idx, Note {
                start: note.start, 
                length: note.length,
                key: note.key,
                velocity: note.velocity,
                channel: note.channel
            });
            
            *offset += 1;

            if is_moving_selected {
                let mut sel = self.selected_notes_ids.lock().unwrap();
                sel[i] = real_idx;
            }
        }

        // register action
        let mut editor_actions = self.editor_actions.borrow_mut();
        match action {
            EditorAction::PlaceNotes(_, _, _) => {
                editor_actions.register_action(EditorAction::PlaceNotes(new_ids, None, curr_track));
            }
            EditorAction::NotesMove(id_override, _, position_deltas, _, update_selected) => {
                // before registering, make sure we actually have moved the notes lol
                let valid_register = {
                    let mut vreg = false;
                    for (tick, key) in position_deltas.iter() {
                        if *tick != 0 || *key != 0 { vreg = true; break; }
                    }
                    vreg
                };

                if valid_register {
                    editor_actions.register_action(EditorAction::NotesMove(
                        if id_override.len() > 0 {
                            id_override
                        } else {
                            old_ids
                        },
                        new_ids,
                        position_deltas,
                        curr_track,
                        update_selected
                    ));
                }
            }
            _ => {}
        }

        {
            //let notes = self.notes.lock().unwrap();
            //let notes = &notes[curr_track as usize][curr_channel as usize];
            println!("{}", notes.is_sorted_by(|a, b| a.start <= b.start));
        }
        

        ghost_notes.clear();
    }

    fn snap_tick(&self, tick: SignedMIDITick) -> SignedMIDITick {
        let snap = self.get_min_snap_tick_length();
        if snap == 1 { return tick; }
        (tick as f32 / snap as f32).round() as SignedMIDITick * (snap as SignedMIDITick)
    }

    fn get_min_snap_tick_length(&self) -> u16 {
        let editor_tool = self.editor_tool.borrow();
        let snap_ratio = editor_tool.snap_ratio;
        if snap_ratio.0 == 0 { return 1; }
        return ((self.ppq as u32 * 4 * snap_ratio.0 as u32)
            / snap_ratio.1 as u32) as u16;
    }

    fn init_selection_box(&mut self, start_tick_pos: MIDITick, start_key_pos: u8) {
        let snapped_tick = self.snap_tick(start_tick_pos as SignedMIDITick) as MIDITick;
        self.selection_range.0 = snapped_tick;
        self.selection_range.1 = snapped_tick;
        self.selection_range.2 = start_key_pos;
        self.selection_range.3 = start_key_pos;

        self.draw_select_box = true;
    }

    fn update_selection_box(&mut self, new_tick_pos: MIDITick, new_key_pos: u8) {
        self.selection_range.1 = self.snap_tick(new_tick_pos as SignedMIDITick) as MIDITick;
        self.selection_range.3 = new_key_pos;
    }

    fn get_selection_range(&self) -> (MIDITick, MIDITick, u8, u8) {
        let (min_tick, max_tick) = {
            if self.selection_range.0 > self.selection_range.1 {
                (self.selection_range.1, self.selection_range.0)
            } else {
                (self.selection_range.0, self.selection_range.1)
            }
        };

        let (min_key, max_key) = {
            if self.selection_range.2 > self.selection_range.3 {
                (self.selection_range.3, self.selection_range.2)
            } else {
                (self.selection_range.2, self.selection_range.3)
            }
        };

        (min_tick, max_tick, min_key, max_key)
    }

    /// Returns (top, left, bottom, right)
    pub fn get_selection_range_ui(&self, ui: &mut Ui) -> ((f32, f32), (f32, f32)) {
       let (min_tick, max_tick) = {
            if self.selection_range.0 > self.selection_range.1 {
                (self.selection_range.1, self.selection_range.0)
            } else {
                (self.selection_range.0, self.selection_range.1)
            }
        };

        // min and max key is inverted because egui said so
        let (max_key, min_key) = {
            if self.selection_range.2 > self.selection_range.3 {
                (self.selection_range.3, self.selection_range.2)
            } else {
                (self.selection_range.2, self.selection_range.3)
            }
        };

        let tl = self.midi_pos_to_ui_pos(ui, min_tick, min_key);
        let br = self.midi_pos_to_ui_pos(ui, max_tick, max_key);

        (tl, br)
    }

    pub fn get_can_draw_selection_box(&self) -> bool {
        self.draw_select_box
    }

    pub fn is_eraser_active(&self) -> bool {
        self.flags & NOTE_EDIT_ERASING != 0
    }

    // helper function to delete notes

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::PlaceNotes(note_ids, notes_deleted, track) => {
                // expect a sorted array
                assert!(note_ids.is_sorted_by(|id1, id2| id1 < id2), "[PLACE_NOTES] Expected a Note ID array in ascending order");
                assert!(notes_deleted.is_some(), "[PLACE_NOTES] Something has gone wrong when trying to place notes.");

                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                //let mut recovered_notes = self.note_undoredo_deleted.pop_front().unwrap(); // get the latest notes that were deleted
                //let mut recovered_notes = self.note_user_deleted.pop_front().unwrap();
                let mut recovered_notes = notes_deleted.take().unwrap();

                assert!(recovered_notes.len() == note_ids.len(), "Expected length of deleted notes to be the same as length of node ids. Instead del_len={} != note_id_len={}", recovered_notes.len(), note_ids.len());

                for ids in note_ids.iter() {
                    let recovered_note = recovered_notes.pop_front().unwrap();
                    notes.insert(*ids, recovered_note);
                }
            },
            EditorAction::DeleteNotes(note_ids, notes_deleted, track) => {
                // expect a sorted array
                assert!(note_ids.is_sorted_by(|id1, id2| id1 < id2), "[DELETE_NOTES] Expected a Note ID array in ascending order");

                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                let mut deleted_notes = VecDeque::with_capacity(note_ids.len());

                for ids in note_ids.iter().rev() {
                    let removed_note = notes.remove(*ids);
                    deleted_notes.push_front(removed_note);
                }
                
                //self.note_user_deleted.push_front(deleted_notes);
                *notes_deleted = Some(deleted_notes);
            },
            EditorAction::LengthChange(note_ids, length_deltas, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];
                for (i, ids) in note_ids.iter().enumerate() {
                    let length = notes[*ids].length as SignedMIDITick;
                    *(notes[*ids].length_mut()) = (length + length_deltas[i]) as MIDITick;
                }
            },
            EditorAction::ChannelChange(note_ids, channel_deltas, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];
                for (i, ids) in note_ids.iter().enumerate() {
                    let channel = notes[*ids].channel as i8;
                    *(notes[*ids].channel_mut()) = (channel + channel_deltas[i]).clamp(0 ,16) as u8;
                }
            },
            EditorAction::NotesMove(new_note_ids, old_note_ids, midi_pos_delta, track, update_selected) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                
                let mut ids_with_pos = old_note_ids.iter().enumerate()
                    .map(|(i, old_id)| (old_id, &new_note_ids[i], &midi_pos_delta[i]))
                    .collect::<Vec<(&_, &_, &_)>>();
                ids_with_pos.sort_by_key(|&(_, new_ids, _)| new_ids);

                // remove notes by descending order
                let mut notes_removed_tmp = Vec::with_capacity(new_note_ids.len());
                for (&old_id, &new_id, &pos_delta) in ids_with_pos.iter().rev() {
                    let mut note = notes.remove(new_id);
                    let (new_start, new_key) = {
                        let mut new_start = note.start as SignedMIDITick + pos_delta.0;
                        let mut new_key = note.key as i16 + pos_delta.1;
                        if new_start < 0 { new_start = 0 }
                        if new_key > 127 { new_key = 127; }
                        if new_key < 0 { new_key = 0; }
                        (new_start as MIDITick, new_key as u8)
                    };
                    note.start = new_start;
                    note.key = new_key;
                    notes_removed_tmp.push((old_id, note));
                }

                // sort
                notes_removed_tmp.sort_by_key(|(id, _)| *id);
                let mut old_ids_sorted = Vec::new();

                for (old_id, note) in notes_removed_tmp.into_iter() {
                    //let insert_idx = bin_search_notes(notes, note.start);
                    let insert_idx = old_id;
                    old_ids_sorted.push(old_id);
                    notes.insert(insert_idx, note);
                }

                if *update_selected {
                    let mut selected = self.selected_notes_ids.lock().unwrap();
                    *selected = old_ids_sorted
                }
            },
            EditorAction::NotesMoveImmediate(note_ids, midi_pos_delta, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                for (i, ids) in note_ids.iter().enumerate() {
                    let start = notes[*ids].start as SignedMIDITick;
                    let key = notes[*ids].key as i16;
                    let (new_start, new_key) = {
                        let mut new_start = start + midi_pos_delta[i].0;
                        let mut new_key = key + midi_pos_delta[i].1;
                        if new_start < 0 {
                            new_start = 0;
                        }
                        if new_key < 0 {
                            new_key = 0;
                        }
                        if new_key > 127 {
                            new_key = 127;
                        }

                        (new_start as MIDITick, new_key as MIDITick)
                    };
                    notes[*ids].start = new_start as MIDITick;
                    notes[*ids].key = new_key as u8;
                }
            },
            EditorAction::Select(note_ids, _) => {
                let mut tmp_sel = self.selected_notes_ids.lock().unwrap();
                tmp_sel.clear();
                for ids in note_ids.iter() {
                    tmp_sel.push(*ids);
                }
            },
            EditorAction::Deselect(note_ids, _) => {
                let mut tmp_sel = self.selected_notes_ids.lock().unwrap();
                for ids in note_ids.iter() {
                    if let Some(index) = tmp_sel.iter().position(|&id| id == *ids) {
                        tmp_sel.remove(index);
                    }
                }
            },
            EditorAction::Bulk(actions) => {
                let mut actions_taken = 0;
                for action in actions.iter_mut().rev() {
                    self.apply_action(action);
                    actions_taken += 1;
                }
                /*while let Some(action) = actions.pop() {
                    self.apply_action(&action);
                    actions_taken += 1;
                }*/
                println!("Actions taken in a bulk action: {}", actions_taken);
            }
            _ => {}
        }
    }

    pub fn update_cursor(&self, ctx: &egui::Context, _ui: &mut Ui) {
        if self.flags & NOTE_EDIT_MOUSE_OVER_UI != 0 {
            ctx.set_cursor_icon(egui::CursorIcon::Default);
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.borrow();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                if self.is_at_note_end {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    return;
                }

                if self.flags & NOTE_EDIT_MOUSE_OVER_NOTE != 0 {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                    return;
                }
            },
            EditorTool::Eraser => {

            },
            EditorTool::Selector => {
                if self.is_at_note_end {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    return;
                }

                if self.flags & NOTE_EDIT_MOUSE_OVER_NOTE != 0 {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                    return;
                }

                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
    }

    pub fn get_flags(&self) -> u16 {
        self.flags
    }

    // probably slow :skull:
    fn update_latest_note_start(&mut self) {
        let notes = self.notes.read().unwrap();
        let mut latest_start: MIDITick = 0;
        for note_track in notes.iter() {
            if note_track.is_empty() { continue; }
            let last_note = note_track.last().unwrap();
            if last_note.start() >= latest_start { latest_start = last_note.start(); }
        }
        self.latest_note_start = latest_start + 38400;
    }
}