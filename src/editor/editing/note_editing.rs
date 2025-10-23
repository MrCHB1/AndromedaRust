use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, RwLock}};
use crate::{app::{main_window::{EditorTool, EditorToolSettings, ToolBarSettings}, rendering::{data_view::DataViewRenderer, RenderManager}}, editor::{self, actions::{EditorAction, EditorActions}, editing::note_editing::note_sequence_funcs::{extract, extract_and_remap_ids, merge_notes, merge_notes_and_return_ids, move_all_notes_by, move_each_note_by, remove_note}, navigation::PianoRollNavigation, project::{project_data::ProjectData, project_manager::ProjectManager}, util::{find_note_at, get_absolute_max_tick_from_ids, get_min_max_ticks_in_selection, get_mouse_midi_pos, get_notes_in_range, MIDITick, SignedMIDITick}}, midi::events::note::Note};
use eframe::egui::{self, Context, CursorIcon, Key, Ui};
use note_edit_flags::*;

const MIN_DRAGGABLE_WIDTH: f32 = 6.0f32;
const END_REGION: f32 = 4.0f32;

pub mod note_edit_flags {
    pub const NOTE_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const NOTE_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const NOTE_EDIT_MOUSE_OVER_NOTE: u16 = 0x2;
    pub const NOTE_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x4;
    pub const NOTE_EDIT_ANY_DIALOG_OPEN: u16 = 0x8;
    pub const NOTE_EDIT_LENGTH_CHANGE: u16 = 0x10;
    pub const NOTE_EDIT_DRAGGING: u16 = 0x20;
    pub const NOTE_EDIT_MULTIEDIT: u16 = 0x40;
    pub const NOTE_EDIT_IS_EDITING: u16 = 0x80;
    pub const NOTE_EDIT_ERASING: u16 = 0x100;
    pub const NOTE_EDIT_SYNTH_PLAY: u16 = 0x200;
}

pub mod note_sequence_funcs;

pub struct GhostNote {
    id: Option<usize>,
    note: Note
}

impl GhostNote {
    #[inline(always)]
    pub fn note_mut(&mut self) -> &mut Note {
        &mut self.note
    }

    #[inline(always)]
    pub fn get_note(&self) -> &Note {
        &self.note
    }

    #[inline(always)]
    pub fn into_note(self) -> Note {
        self.note
    }

    #[inline(always)]
    pub fn set_note(&mut self, note: Note) {
        self.note = note
    }
}

#[derive(Default)]
struct NoteEditMouseInfo {
    mouse_midi_pos: (MIDITick, u8),
    mouse_midi_pos_rounded: (MIDITick, u8),
    last_mouse_click_pos: (MIDITick, u8),
    last_clicked_note_idx: Option<usize>,
    last_clicked_note_pos: (MIDITick, u8),
    note_hover_idx: Option<usize>,
    is_at_note_end: bool,
}

#[derive(Default)]
pub struct NoteEditing {
    notes: Arc<RwLock<Vec<Vec<Note>>>>,
    ghost_notes: Arc<Mutex<Vec<Note>>>,
    selected_note_ids: Arc<Mutex<Vec<usize>>>,
    nav: Arc<Mutex<PianoRollNavigation>>,
    mouse_info: NoteEditMouseInfo,
    editor_tool: Rc<RefCell<EditorToolSettings>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    toolbar_settings: Rc<RefCell<ToolBarSettings>>,

    // for sending to renderers
    render_manager: Arc<Mutex<RenderManager>>,
    data_view_renderer: Option<Arc<Mutex<DataViewRenderer>>>,

    // for temporary note movement idk
    note_old_positions: Vec<(MIDITick, u8)>,
    note_old_lengths: Vec<(usize, MIDITick)>,
    pub ppq: u16,

    // notes clipboard
    notes_clipboard: Vec<Note>,

    // other
    pub latest_note_start: MIDITick,
    pub selection_range: (MIDITick, MIDITick, u8, u8),
    draw_select_box: bool,

    flags: u16,
}

impl NoteEditing {
    pub fn new(
        notes: &Arc<RwLock<Vec<Vec<Note>>>>,
        nav: &Arc<Mutex<PianoRollNavigation>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        editor_actions: &Rc<RefCell<EditorActions>>,
        toolbar_settings: &Rc<RefCell<ToolBarSettings>>,

        render_manager: &Arc<Mutex<RenderManager>>,
        data_view_renderer: &Arc<Mutex<DataViewRenderer>>,
    ) -> Self {

        {
            let notes = notes.read().unwrap();
            assert!(!notes.is_empty(), "notes have to be populated");
        }

        Self {
            notes: notes.clone(),
            ghost_notes: Arc::new(Mutex::new(Vec::new())),
            selected_note_ids: Arc::new(Mutex::new(Vec::new())),
            notes_clipboard: Vec::new(),
            nav: nav.clone(),
            mouse_info: NoteEditMouseInfo::default(),
            editor_tool: editor_tool.clone(),
            editor_actions: editor_actions.clone(),
            toolbar_settings: toolbar_settings.clone(),

            note_old_positions: Vec::new(),
            note_old_lengths: Vec::new(),

            render_manager: render_manager.clone(),
            data_view_renderer: Some(data_view_renderer.clone()),
            latest_note_start: 38400,
            ppq: 960,
            selection_range: (0, 0, 0, 0),
            draw_select_box: false,

            flags: NOTE_EDIT_FLAGS_NONE
        }
    }

    pub fn update_from_ui(&mut self, ui: &mut Ui) {
        let (mouse_midi_pos, mouse_midi_pos_rounded) = get_mouse_midi_pos(ui, &self.nav);

        {
            let mouse_info = &mut self.mouse_info;
            mouse_info.mouse_midi_pos = mouse_midi_pos;
            mouse_info.mouse_midi_pos_rounded = mouse_midi_pos_rounded;
        }

        // self.set_flag(NOTE_EDIT_MOUSE_OVER_NOTE, false);
        self.disable_flag(NOTE_EDIT_MOUSE_OVER_NOTE);
        if self.get_flag(NOTE_EDIT_MOUSE_OVER_UI) {
            self.mouse_info.note_hover_idx = None;
            return;
        }

        let curr_track = self.get_current_track();

        let mouse_note_hover_idx = {
            let notes = self.notes.read().unwrap();
            if notes.is_empty() { None }
            else {
                let notes = &notes[curr_track as usize];
                find_note_at(notes, mouse_midi_pos.0, mouse_midi_pos.1)
            }
        };

        self.mouse_info.is_at_note_end = if let Some(idx) = mouse_note_hover_idx {
            self.enable_flag(NOTE_EDIT_MOUSE_OVER_NOTE);

            let rect = ui.min_rect();

            let nav = self.nav.lock().unwrap();
            let notes = self.notes.read().unwrap();
            let notes = &notes[curr_track as usize];
            let note = &notes[idx];

            let note_screen_width =
                (note.length() as f32 / nav.zoom_ticks_smoothed) * rect.width();
            let dist_to_end = 
                (note.end() as f32 - mouse_midi_pos.0 as f32) / nav.zoom_ticks_smoothed * rect.width();

            if note_screen_width > MIN_DRAGGABLE_WIDTH {
                dist_to_end >= 0.0 && dist_to_end < END_REGION
            } else {
                false
            }
        } else {
            false
        };
        
        self.mouse_info.note_hover_idx = mouse_note_hover_idx;
    }

    // ======== MOUSE EVENT FUNCTIONS ========

    pub fn on_mouse_down(&mut self) {
        if self.get_flag(NOTE_EDIT_MOUSE_OVER_UI) {
            self.enable_flag(NOTE_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(NOTE_EDIT_ANY_DIALOG_OPEN) { return; }

        self.update_clicked_note();
        self.update_latest_note_start();

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                self.pencil_mouse_down();
            },
            EditorTool::Eraser => {
                self.eraser_mouse_down();
            },
            EditorTool::Selector => {
                self.select_mouse_down();
            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.get_flag(NOTE_EDIT_MOUSE_OVER_UI) {
            self.enable_flag(NOTE_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(NOTE_EDIT_ANY_DIALOG_OPEN) { return; }

        self.update_clicked_note();
        self.update_latest_note_start();

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil | EditorTool::Eraser => {
                self.eraser_mouse_down();
            },
            _ => {}
        }
    }

    pub fn on_mouse_move(&mut self) {
        if self.get_flag(NOTE_EDIT_MOUSE_DOWN_ON_UI) { return; }
        if self.get_flag(NOTE_EDIT_ANY_DIALOG_OPEN | NOTE_EDIT_MOUSE_OVER_UI) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                self.pencil_mouse_move();
            },
            EditorTool::Eraser => {
                self.eraser_mouse_move();
            },
            EditorTool::Selector => {
                self.select_mouse_move();
            }
        }
    }

    pub fn on_mouse_up(&mut self) {
        if self.get_flag(NOTE_EDIT_MOUSE_DOWN_ON_UI) {
            self.disable_flag(NOTE_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(NOTE_EDIT_MOUSE_OVER_UI | NOTE_EDIT_ANY_DIALOG_OPEN) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => {
                self.pencil_mouse_up();
            },
            EditorTool::Eraser => {
                self.eraser_mouse_up();
            },
            EditorTool::Selector => {
                self.select_mouse_up();
            }
        }
    }

    pub fn on_key_down(&mut self, ui: &mut Ui) {
        if !self.get_flag(NOTE_EDIT_ANY_DIALOG_OPEN | NOTE_EDIT_MOUSE_OVER_UI) {
            let curr_track = self.get_current_track();
            if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Copy))) {
                println!("Copied");
                self.copy_notes(curr_track);
            }

            if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Cut))) {
                println!("Cut");
                self.cut_selected_notes(curr_track);
            }

            if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Paste(_)))) {
                println!("Pasted");
                self.paste_notes(curr_track);
            }

            if ui.input(|i| i.key_pressed(Key::D) && i.modifiers.command) {
                self.duplicate_selected_notes();
            }
        }

        if ui.input(|i| i.key_pressed(Key::Delete)) {
            let sel_ids = {
                let mut selected_ids = self.selected_note_ids.lock().unwrap();
                std::mem::take(&mut *selected_ids)
            };

            self.delete_notes_no_remap(sel_ids);
        }
    }

    fn update_clicked_note(&mut self) {
        let curr_track = self.get_current_track();

        let mouse_info = &mut self.mouse_info;
        mouse_info.last_clicked_note_idx = mouse_info.note_hover_idx;
        mouse_info.last_mouse_click_pos = mouse_info.mouse_midi_pos;
        
        if let Some(clicked_idx) = mouse_info.last_clicked_note_idx {
            let notes = self.notes.read().unwrap();
            let note = &notes[curr_track as usize][clicked_idx];
            mouse_info.last_clicked_note_pos = (note.start(), note.key());
        }
    }

    fn get_clicked_note_idx(&mut self) -> Option<usize> {
        self.mouse_info.last_clicked_note_idx
    }

    // ======== PENCIL TOOL STUFF ========

    fn pencil_mouse_down(&mut self) {
        let mut can_show_ghost_notes = false;

        // we are over a note
        if let Some(clicked_idx) = self.get_clicked_note_idx() {
            {
                let curr_track = self.get_current_track();
                let notes = self.notes.read().unwrap();
                let note = &notes[curr_track as usize][clicked_idx];

                self.update_toolbar_settings_from_note(note);
            }

            self.disable_flag(NOTE_EDIT_LENGTH_CHANGE | NOTE_EDIT_DRAGGING | NOTE_EDIT_MULTIEDIT);
            
            let is_multi = {
                let mut sel = self.selected_note_ids.lock().unwrap();
                if !sel.is_empty() && sel.contains(&clicked_idx) && sel.len() > 1 {
                    true
                } else {
                    sel.clear();
                    false
                }
            };
            // self.disable_flag(NOTE_EDIT_DRAGGING);
            // self.disable_flag(flag);

            // let is_multi = self.get_flag(NOTE_EDIT_MULTIEDIT);

            if self.mouse_info.is_at_note_end {
                self.enable_flag(NOTE_EDIT_LENGTH_CHANGE);

                let sel_lengths = {
                    let selected_ids = self.selected_note_ids.lock().unwrap();
                    let ids = if is_multi {
                        &*selected_ids
                    } else {
                        &vec![clicked_idx]
                    };
                    self.get_note_lengths(ids)
                };

                self.note_old_lengths = sel_lengths;
            } else {
                self.enable_flag(NOTE_EDIT_DRAGGING);

                let sel_positions = {
                    let selected_ids = self.selected_note_ids.lock().unwrap();
                    let ids = if is_multi {
                        &*selected_ids
                    } else {
                        &vec![clicked_idx]
                    };
                    self.get_note_positions(ids)
                };

                self.note_old_positions = sel_positions;
                can_show_ghost_notes = true;

                if is_multi {
                    self.selected_notes_to_ghost_notes();
                } else {
                    self.note_id_as_first_ghost_note(clicked_idx);
                }

                // not dragging, so enable flag for playing notes
                self.enable_flag(NOTE_EDIT_SYNTH_PLAY);
            }

            if is_multi {
                self.enable_flag(NOTE_EDIT_MULTIEDIT);
            } else {
                self.update_render_selected_notes();
            }
        } else {
            self.clear_selected();
            self.update_first_ghost_note();
            can_show_ghost_notes = true;
            self.enable_flag(NOTE_EDIT_IS_EDITING);

            // making a new note, so enable flag for playing notes
            self.enable_flag(NOTE_EDIT_SYNTH_PLAY);
        }

        if can_show_ghost_notes {
            self.show_ghost_notes();
        }
    }

    fn pencil_mouse_move(&mut self) {
        if self.get_flag(NOTE_EDIT_IS_EDITING) {
            self.update_first_ghost_note();
        } else if self.get_flag(NOTE_EDIT_DRAGGING) {
            self.offset_ghost_notes_tmp();
        } else if self.get_flag(NOTE_EDIT_LENGTH_CHANGE) {
            self.offset_notes_length_tmp();
        }
    }

    fn pencil_mouse_up(&mut self) {
        if self.get_flag(NOTE_EDIT_IS_EDITING) {
            self.hide_ghost_notes();
            self.apply_ghost_place_notes();
            self.disable_flag(NOTE_EDIT_IS_EDITING);
            self.disable_flag(NOTE_EDIT_SYNTH_PLAY);
        } else if self.get_flag(NOTE_EDIT_DRAGGING) {
            self.hide_ghost_notes();

            let ghost_deltas = self.get_ghost_notes_pos_delta();
            self.apply_ghost_move_notes(ghost_deltas);

            self.disable_flag(NOTE_EDIT_DRAGGING);
            self.disable_flag(NOTE_EDIT_SYNTH_PLAY);
        } else if self.get_flag(NOTE_EDIT_LENGTH_CHANGE) {
            self.apply_note_length_change();
            self.disable_flag(NOTE_EDIT_LENGTH_CHANGE);
        }
    }

    // ======== SELECT TOOL STUFF ========

    fn select_mouse_down(&mut self) {
        self.disable_flag(NOTE_EDIT_DRAGGING | NOTE_EDIT_LENGTH_CHANGE | NOTE_EDIT_MULTIEDIT);

        if let Some(clicked_idx) = self.get_clicked_note_idx() {
            let should_modify_selected = {
                let selected_ids = self.selected_note_ids.lock().unwrap();
                !selected_ids.is_empty() && selected_ids.contains(&clicked_idx)
            };

            if should_modify_selected {
                self.enable_flag(NOTE_EDIT_MULTIEDIT);
            }

            if self.mouse_info.is_at_note_end {
                self.enable_flag(NOTE_EDIT_LENGTH_CHANGE);

                let sel_lengths = {
                    let selected_ids = self.selected_note_ids.lock().unwrap();
                    let ids = if should_modify_selected {
                        &*selected_ids
                    } else {
                        &vec![clicked_idx]
                    };
                    self.get_note_lengths(ids)
                };

                self.note_old_lengths = sel_lengths;
            } else {
                self.enable_flag(NOTE_EDIT_DRAGGING);

                let sel_positions = {
                    let selected_ids = self.selected_note_ids.lock().unwrap();
                    let ids = if should_modify_selected {
                        &*selected_ids
                    } else {
                        &vec![clicked_idx]
                    };
                    self.get_note_positions(ids)
                };

                self.note_old_positions = sel_positions;

                if should_modify_selected {
                    self.selected_notes_to_ghost_notes();
                } else {
                    self.clear_selected();
                    self.note_id_as_first_ghost_note(clicked_idx);
                }
            }

            self.show_ghost_notes();
        } else {
            self.init_selection_box(self.mouse_info.mouse_midi_pos);
        }
    }

    fn select_mouse_move(&mut self) {
        if self.get_flag(NOTE_EDIT_DRAGGING) {
            self.offset_ghost_notes_tmp();
        } else if self.get_flag(NOTE_EDIT_LENGTH_CHANGE) {
            self.offset_notes_length_tmp();
        } else {
            if self.draw_select_box {
                let mouse_info = &self.mouse_info;
                self.update_selection_box(mouse_info.mouse_midi_pos);
            }
        }
    }

    fn select_mouse_up(&mut self) {
        if self.get_flag(NOTE_EDIT_DRAGGING) {
            self.hide_ghost_notes();
            let ghost_deltas = self.get_ghost_notes_pos_delta();
            self.apply_ghost_move_notes(ghost_deltas);
            self.disable_flag(NOTE_EDIT_DRAGGING);
        } else if self.get_flag(NOTE_EDIT_LENGTH_CHANGE) {
            self.apply_note_length_change();
            self.disable_flag(NOTE_EDIT_LENGTH_CHANGE);
        } else {
            self.draw_select_box = false;

            let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();
            let curr_track = self.get_current_track();

            let notes = self.notes.read().unwrap();
            let notes = &notes[curr_track as usize];

            let selected = get_notes_in_range(notes, min_tick, max_tick, min_key, max_key, true);

            {
                let mut sel = self.selected_note_ids.lock().unwrap();
                let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();

                if !selected.is_empty() {
                    editor_actions.register_action(EditorAction::Select(selected.clone(), curr_track));
                } else if !sel.is_empty() {
                    editor_actions.register_action(EditorAction::Deselect(std::mem::take(&mut sel), curr_track));
                }

                *sel = selected;
            }

            self.update_render_selected_notes();
        }
    }

    // ======== SELECTED NOTES/TOOL HELPER FUNCTIONS ========

    fn offset_ghost_notes_tmp(&mut self) {
        let (clicked_note_start, clicked_note_key) = self.get_clicked_note_pos();
        let mouse_info = &self.mouse_info;

        if self.get_flag(NOTE_EDIT_MULTIEDIT) {
            let offset_start = {
                let mouse_delta_ticks = mouse_info.mouse_midi_pos.0 as SignedMIDITick
                    -mouse_info.last_mouse_click_pos.0 as SignedMIDITick;
                
                let snapped_delta = self.snap_tick(mouse_delta_ticks);
                snapped_delta
            };

            let offset_key = {
                let key_offs = mouse_info.mouse_midi_pos.1 as i16 - clicked_note_key as i16;
                key_offs
            };

            self.offset_ghost_notes((offset_start, offset_key));
        } else {
            // offset ghost note based on difference between clicked note tick and mouse pos
            let ghost_start = {
                let mouse_delta_ticks = mouse_info.mouse_midi_pos.0 as SignedMIDITick
                    -mouse_info.last_mouse_click_pos.0 as SignedMIDITick;
                
                let snapped_delta = self.snap_tick(mouse_delta_ticks);

                let ghost_start = clicked_note_start as SignedMIDITick + snapped_delta;
                let ghost_start = if ghost_start < 0 { 0 } else { ghost_start as MIDITick };
                ghost_start
            };

            let ghost_key = {
                let key_offs = clicked_note_key as i16 - mouse_info.last_clicked_note_pos.1 as i16;

                let ghost_key = mouse_info.mouse_midi_pos.1 as i16 + key_offs;
                if ghost_key < 0 { 0 }
                else if ghost_key > 127 { 127 }
                else { ghost_key as u8 }
            };

            self.set_first_ghost_note_pos(ghost_start, ghost_key);
        }
    }

    fn offset_notes_length_tmp(&mut self) {
        let mouse_info = &self.mouse_info;

        // if self.get_flag(NOTE_EDIT_MULTIEDIT) {
            let offset_start = {
                let mouse_delta_ticks = mouse_info.mouse_midi_pos.0 as SignedMIDITick
                    -mouse_info.last_mouse_click_pos.0 as SignedMIDITick;
                
                let snapped_delta = self.snap_tick(mouse_delta_ticks);
                snapped_delta
            };
            
            self.offset_note_lengths(offset_start);
        // }
    }

    fn init_selection_box(&mut self, start_pos: (MIDITick, u8)) {
        let snapped_tick = self.snap_tick(start_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range = (snapped_tick, snapped_tick, start_pos.1, start_pos.1);
        self.draw_select_box = true;
    }

    fn update_selection_box(&mut self, new_pos: (MIDITick, u8)) {
        self.selection_range.1 = self.snap_tick(new_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range.3 = new_pos.1;
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

    fn clear_selected(&mut self) {
        {
            let mut selected_ids = self.selected_note_ids.lock().unwrap();
            selected_ids.clear();
        }

        self.update_render_selected_notes();
    }

    // ======== ERASER TOOL STUFF ========

    fn eraser_mouse_down(&mut self) {
        if let Some(clicked_note_idx) = self.get_clicked_note_idx() {
            // immediately delete the note that's being clicked
            self.delete_notes(vec![clicked_note_idx]);
        } else {
            self.enable_flag(NOTE_EDIT_ERASING);
            self.init_selection_box(self.mouse_info.mouse_midi_pos);
        }
    }

    fn eraser_mouse_move(&mut self) {
        if self.draw_select_box {
            self.update_selection_box(self.mouse_info.mouse_midi_pos);
        }
    }

    fn eraser_mouse_up(&mut self) {
        self.disable_flag(NOTE_EDIT_ERASING);
        self.draw_select_box = false;

        let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();
        let curr_track = self.get_current_track();

        let mut selected = {
            let notes = self.notes.read().unwrap();
            let notes = &notes[curr_track as usize];

            get_notes_in_range(notes, min_tick, max_tick, min_key, max_key, true)
        };

        if !selected.is_empty() {
            self.delete_notes(std::mem::take(&mut selected));
        }
    }

    // ======== NOTE STUFF ========

    pub fn get_notes(&self) -> &Arc<RwLock<Vec<Vec<Note>>>> {
        &self.notes
    }

    pub fn get_selected_note_ids(&self) -> &Arc<Mutex<Vec<usize>>> {
        &self.selected_note_ids
    }

    /// Only call this when mouse was already hovering over note.
    pub fn get_clicked_note_pos(&self) -> (MIDITick, u8) {
        self.mouse_info.last_clicked_note_pos
    }

    // ======== MISC. NOTE STUFF ========

    /// Gets each note from [`ids`]'s position
    pub fn get_note_positions(&self, ids: &Vec<usize>) -> Vec<(MIDITick, u8)> {
        // let mut saved_positions: Vec<(MIDITick, u8)> = Vec::with_capacity(ids.len());

        let curr_track = self.get_current_track();
        let notes = self.notes.read().unwrap();
        let notes = &notes[curr_track as usize];

        ids.iter().map(|&id| {
            let note = &notes[id];
            (note.start(), note.key())
        }).collect()
    }

    pub fn get_note_lengths(&self, ids: &Vec<usize>) -> Vec<(usize, MIDITick)> {
        // let mut saved_lengths: Vec<MIDITick> = Vec::with_capacity(ids.len());

        let curr_track = self.get_current_track();
        let notes = self.notes.read().unwrap();
        let notes = &notes[curr_track as usize];

        ids.iter().map(|&id| {
            let note = &notes[id];
            (id, note.length())
        }).collect()
    }

    fn offset_note_lengths(&mut self, length_delta: SignedMIDITick) {
        let curr_track = self.get_current_track();

        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[curr_track as usize];

        assert!(!self.note_old_lengths.is_empty(), "old note lengths must be populated");

        for old_length in self.note_old_lengths.iter() {
            let (note_id, old_length) = *old_length;
            let new_length = old_length as SignedMIDITick + length_delta;
            
            let note = &mut notes[note_id];
            *(note.length_mut()) = if new_length < 1 { 1 }
            else { new_length as MIDITick }
        }
    }

    fn apply_note_length_change(&mut self) {
        let curr_track = self.get_current_track();

        let notes = self.notes.read().unwrap();
        let notes = &notes[curr_track as usize];

        let old_length = std::mem::take(&mut self.note_old_lengths);
        let (note_ids, length_deltas): (Vec<usize>, Vec<SignedMIDITick>) = old_length.into_iter()
            .map(|(id, length)| {
                let note = &notes[id];
                (id, note.length() as SignedMIDITick - length as SignedMIDITick)
            }).collect();
        
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::LengthChange(note_ids, length_deltas, curr_track));
    }

    // ======== GHOST NOTE STUFF ========
    fn selected_notes_to_ghost_notes(&mut self) {
        let curr_track = self.get_current_track();

        // let mut new_ghost_notes = Vec::with_capacity(selected_notes.len());
        let old_notes = {
            let mut notes = self.notes.write().unwrap();
            let notes = &mut notes[curr_track as usize];
            std::mem::take(notes)
        };
        
        let (tmp_ghosts, new_notes) = {
            let selected = self.selected_note_ids.lock().unwrap();
            extract(old_notes, &selected)
            // extract_notes(old_notes, &selected)
        };

        self.set_notes_in_track(curr_track, new_notes);
        
        let selected = {
            let mut selected = self.selected_note_ids.lock().unwrap();
            std::mem::take(&mut *selected)
        };

        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        *ghost_notes = tmp_ghosts;
        // *ghost_notes = self.notes_into_ghost_notes(tmp_ghosts, selected);
        println!("{}", ghost_notes.len());
    }

    fn note_id_as_first_ghost_note(&mut self, id: usize) {
        let curr_track = self.get_current_track();

        let mut notes = self.notes.write().unwrap();
        let track = &mut notes[curr_track as usize];
        
        let note = remove_note(track, id);

        // put removed note to ghost notes
        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        if ghost_notes.is_empty() {
            ghost_notes.push(note);
            // make it a ghost note
            // let ghost_note = GhostNote { id: Some(id), note };
            // ghost_notes.push(ghost_note);
        } else {
            ghost_notes[0] = note;
            //let ghost_note = ghost_notes.get_mut(0).unwrap();
            //ghost_note.set_note(note);
        }
    }

    fn update_first_ghost_note(&mut self) {
        let (gn_start, gn_key) = {
            let (mouse_tick, mouse_key) = self.mouse_info.mouse_midi_pos;
            let start = mouse_tick;
            let key = mouse_key;
            (start, key)
        };

        let gn_start = self.snap_tick(gn_start as SignedMIDITick) as MIDITick;

        self.set_first_ghost_note_pos(gn_start, gn_key);
    }

    fn set_first_ghost_note_pos(&mut self, start: MIDITick, key: u8) {
        let (gn_channel, gn_length, gn_velocity) = self.get_tbs_values();

        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        if ghost_notes.is_empty() {
            ghost_notes.push(Note {
                start,
                length: gn_length,
                channel: gn_channel,
                key,
                velocity: gn_velocity
            });
            /*ghost_notes.push(GhostNote {
                id: None,
                note: Note { start, length: gn_length, channel: gn_channel, key, velocity: gn_velocity }
            });*/
        } else {
            let ghost_note = &mut ghost_notes[0];
            // let gn = ghost_note.note_mut();

            *(ghost_note.start_mut()) = start;
            *(ghost_note.length_mut()) = gn_length;
            *(ghost_note.channel_mut()) = gn_channel;
            *(ghost_note.key_mut()) = key;
            *(ghost_note.velocity_mut()) = gn_velocity;
        }
    }

    fn offset_ghost_notes(&mut self, pos_delta: (SignedMIDITick, i16)) {
        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        assert!(!self.note_old_positions.is_empty(), "can't call offset_ghost_notes, note_old_positions is empty");
        assert!(ghost_notes.len() == self.note_old_positions.len(), "can't call offset_ghost_notes, length of ghost notes != length of old note positions");

        for (i, old_pos) in self.note_old_positions.iter().enumerate() {
            let ghost_start = old_pos.0 as SignedMIDITick + pos_delta.0;
            let ghost_key = old_pos.1 as i16 + pos_delta.1;
            
            let gn = &mut ghost_notes[i];
            *(gn.start_mut()) = if ghost_start < 0 {
                0
            } else {
                ghost_start as MIDITick
            };

            *(gn.key_mut()) = if ghost_key < 0 {
                0
            } else if ghost_key > 127 {
                127
            } else {
                ghost_key as u8
            };
        }
    }

    fn show_ghost_notes(&mut self) {
        {
            let mut render_manager = self.render_manager.lock().unwrap();
            let curr_renderer = render_manager.get_active_renderer();
            curr_renderer.lock().unwrap().set_ghost_notes(self.ghost_notes.clone());
        }

        {
            let data_view_renderer = self.data_view_renderer.as_ref().unwrap();
            let mut data_view_renderer = data_view_renderer.lock().unwrap();
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
            let data_view_renderer = self.data_view_renderer.as_ref().unwrap();
            let mut data_view_renderer = data_view_renderer.lock().unwrap();
            data_view_renderer.clear_ghost_notes();
        }
    }

    fn ghost_notes_into_notes(&mut self) -> Vec<Note> {
        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        std::mem::take(&mut *ghost_notes)
        //let ghost_notes = std::mem::take(&mut *ghost_notes);
        //ghost_notes.into_iter()
        //    .map(|gn| gn.into_note())
        //    .collect()
    }

    fn notes_into_ghost_notes(&self, notes: Vec<Note>, orig_ids: Vec<usize>) -> Vec<GhostNote> {
        notes.into_iter().zip(orig_ids.clone()).map(|(note, id)| {
            GhostNote {
                id: Some(id),
                note
            }
        }).collect()
    }

    fn merge_ghost_notes(&mut self, track: u16) -> Vec<usize> {
        let ghost_notes = self.ghost_notes_into_notes();
        let curr_notes_track = self.take_notes_curr_track();
        
        let (merged, ids) = merge_notes_and_return_ids(curr_notes_track, ghost_notes);
        self.set_notes_in_track(track, merged);

        ids
    }

    fn apply_ghost_place_notes(&mut self) {
        let curr_track = self.get_current_track();

        let ids = self.merge_ghost_notes(curr_track);
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::PlaceNotes(ids, None, curr_track));
    }

    fn apply_ghost_move_notes(&mut self, pos_deltas: Vec<(SignedMIDITick, i16)>) {
        let curr_track = self.get_current_track();

        let ids = self.merge_ghost_notes(curr_track);
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        
        let is_editing_selected = if pos_deltas.len() > 1 {
            {
                let mut selected_notes = self.selected_note_ids.lock().unwrap();
                *selected_notes = ids.clone();
            }
            self.update_render_selected_notes();
            true
        } else {
            false
        };

        editor_actions.register_action(EditorAction::NotesMove(ids, pos_deltas, curr_track, is_editing_selected));
    }

    fn get_ghost_notes_pos_delta(&self) -> Vec<(SignedMIDITick, i16)> {
        let ghost_notes = self.ghost_notes.lock().unwrap();
        ghost_notes.iter().enumerate().map(|(i, note)| {
            //let note = ghost_note.get_note();
            let delta_pos = {
                let old_pos = self.note_old_positions[i];
                let new_pos = (note.start(), note.key());
                (new_pos.0 as SignedMIDITick - old_pos.0 as SignedMIDITick, new_pos.1 as i16 - old_pos.1 as i16)
            };
            delta_pos
        }).collect()
    }

    // ======== NOTE HELPER FUNCTIONS ========

    fn take_notes_curr_track(&mut self) -> Vec<Note> {
        let curr_track = self.get_current_track();
        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[curr_track as usize];
        std::mem::take(notes)
    }

    pub fn take_notes_in_track(&mut self, track: u16) -> Vec<Note> {
        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[track as usize];
        std::mem::take(notes)
    }

    pub fn set_notes_in_track(&mut self, track: u16, notes_: Vec<Note>) {
        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[track as usize];
        *notes = notes_;
    }

    pub fn duplicate_selected_notes(&mut self) {
        let curr_track = self.get_current_track();
        
        let old_selected = {
            let mut selected_ids = self.selected_note_ids.lock().unwrap();
            std::mem::take(&mut *selected_ids)
        };

        let new_selected = self.duplicate_notes(curr_track, &old_selected);

        {
            let mut selected_ids = self.selected_note_ids.lock().unwrap();
            *selected_ids = new_selected;
        }
        self.update_render_selected_notes();
    }

    pub fn duplicate_notes(&mut self, track: u16, ids: &Vec<usize>) -> Vec<usize> {
        let copied_notes = self.clone_notes(track, ids);

        let old_notes = self.take_notes_in_track(track);

        let min_tick = old_notes[ids[0]].start();
        let max_tick = get_absolute_max_tick_from_ids(&old_notes, ids).unwrap();
        let moved = move_all_notes_by(copied_notes, (max_tick as SignedMIDITick - min_tick as SignedMIDITick, 0));

        let (merged, dupe_ids) = merge_notes_and_return_ids(old_notes, moved);
        
        {
            let mut notes = self.notes.write().unwrap();
            let notes = &mut notes[track as usize];
            *notes = merged
        }

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::PlaceNotes(dupe_ids.clone(), None, track));

        dupe_ids
    }

    pub fn clone_notes(&self, track: u16, ids: &[usize]) -> Vec<Note> {
        let notes = self.notes.read().unwrap();
        let notes = &notes[track as usize];

        let copied = ids.iter().map(|&id| { 
            let note = &notes[id];
            note.clone()
        }).collect();

        copied
    }

    pub fn copy_notes(&mut self, track: u16) {
        let selected = self.selected_note_ids.lock().unwrap();
        if selected.is_empty() { println!("Nothing copied."); return; }
        let notes_clipboard = self.clone_notes(track, &*selected);
        self.notes_clipboard = notes_clipboard;
    }

    pub fn cut_selected_notes(&mut self, track: u16) {
        let old_notes = self.take_notes_in_track(track);
        
        let selected = {
            let mut selected = self.selected_note_ids.lock().unwrap();
            std::mem::take(&mut *selected)
        };
        
        let (notes_to_cut, new_notes) = extract(old_notes, &selected);
        self.update_render_selected_notes();
        self.set_notes_in_track(track, new_notes);
        
        self.notes_clipboard = notes_to_cut.clone();

        // register as "notes deleted"
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::DeleteNotes(selected, Some(notes_to_cut), track));
    }

    pub fn paste_notes_offset(&mut self, track: u16, tick_pos: MIDITick) {
        // copy notes from clipboard
        let mut copied_notes = self.notes_clipboard.clone();
        let first_tick = copied_notes[0].start();
        for note in copied_notes.iter_mut() {
            let note_paste_tick = note.start() - first_tick;
            (*note.start_mut()) = note_paste_tick + tick_pos;
        }

        // merge them to track
        let old_notes = self.take_notes_in_track(track);
        let (new_notes, new_ids) = merge_notes_and_return_ids(old_notes, copied_notes);
        self.set_notes_in_track(track, new_notes);

        // register as "placed notes"
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::PlaceNotes(new_ids, None, track));
    }

    pub fn paste_notes(&mut self, track: u16) {
        let copied_notes = self.notes_clipboard.clone();

        let old_notes = self.take_notes_in_track(track);
        let (new_notes, new_ids) = merge_notes_and_return_ids(old_notes, copied_notes);
        self.set_notes_in_track(track, new_notes);

        // select the pasted notes
        {
            {
                let mut selected_ids = self.selected_note_ids.lock().unwrap();
                *selected_ids = new_ids.clone();
            }
            self.update_render_selected_notes();
        }

        // register as "placed notes"
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::PlaceNotes(new_ids, None, track));
    }

    pub fn delete_notes(&mut self, ids: Vec<usize>) {
        let curr_track = self.get_current_track();
        
        let old_notes = self.take_notes_in_track(curr_track);

        let mut selected = self.selected_note_ids.lock().unwrap();
        // TODO: also extract any selected notes that were deleted
        let (deleted_notes, new_notes, selected_) = extract_and_remap_ids(old_notes, &ids, std::mem::take(&mut *selected));
        *selected = selected_;
        drop(selected);
        self.update_render_selected_notes();
        
        self.set_notes_in_track(curr_track, new_notes);
        
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::DeleteNotes(ids, Some(deleted_notes), curr_track));
    }

    pub fn delete_notes_no_remap(&mut self, ids: Vec<usize>) {
        let curr_track = self.get_current_track();
        
        let old_notes = self.take_notes_in_track(curr_track);

        let (deleted_notes, new_notes) = extract(old_notes, &ids);
        self.set_notes_in_track(curr_track, new_notes);
        
        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::DeleteNotes(ids, Some(deleted_notes), curr_track));
    }

    // ======== EDITOR ACTION FUNCTIONS ========

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::PlaceNotes(_, notes_deleted, track) => {
                assert!(notes_deleted.is_some(), "[PLACE_NOTES] Something has gone wrong while undoing/redoing note deletion.");
                
                let recovered_notes = notes_deleted.take().unwrap();
                let old_notes = self.take_notes_in_track(*track);

                // let (merged, ids) = merge_notes_and_return_ids(old_notes, recovered_notes);
                let merged = merge_notes(old_notes, recovered_notes);
                self.set_notes_in_track(*track, merged);
            },
            EditorAction::DeleteNotes(note_ids, notes_deleted, track) => {
                let old_notes = self.take_notes_in_track(*track);

                let mut selected_ids = self.selected_note_ids.lock().unwrap();
                let old_sel_ids = std::mem::take(&mut *selected_ids);
                let (deleted, new_notes, new_ids) = extract_and_remap_ids(old_notes, &note_ids, old_sel_ids);
                *selected_ids = new_ids;
                drop(selected_ids);

                self.set_notes_in_track(*track, new_notes);

                *notes_deleted = Some(deleted);
            },
            EditorAction::NotesMove(note_ids, delta_pos, track, selected) => {
                let old_notes = self.take_notes_in_track(*track);
                let (notes_to_move, old_notes) = extract(old_notes, &note_ids);

                let notes_with_dt = move_each_note_by(notes_to_move, &delta_pos);
                let (notes_to_move, notes_dt): (_, Vec<_>) = notes_with_dt.into_iter().unzip();

                let (merged, new_ids) = merge_notes_and_return_ids(old_notes, notes_to_move);
                self.set_notes_in_track(*track, merged);

                if *selected {
                    {
                        let mut selected_ids = self.selected_note_ids.lock().unwrap();
                        *selected_ids = new_ids.clone();
                    }
                }

                *note_ids = new_ids;
                *delta_pos = notes_dt;
            },
            EditorAction::ChannelChange(note_ids, delta_channel, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                for (i, &id) in note_ids.iter().enumerate() {
                    let note = &mut notes[id];
                    let dt_channel = &mut delta_channel[i];
                    let mut new_channel = note.channel() as i8 + *dt_channel;
                    if new_channel < 0 {
                        new_channel = 0;
                    }
                    if new_channel > 15 {
                        new_channel = 15;
                    }

                    *dt_channel = new_channel - note.channel() as i8;
                    *(notes[id].channel_mut()) = new_channel as u8;
                }
            },
            EditorAction::LengthChange(note_ids, delta_length, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                for (i, &id) in note_ids.iter().enumerate() {
                    let note = &mut notes[id];
                    let dt_length = &mut delta_length[i];
                    let mut new_length = (note.length() as SignedMIDITick).saturating_add(*dt_length);
                    if new_length < 1 { new_length = 1; }

                    *dt_length = new_length - note.length() as SignedMIDITick;
                    *(notes[id].length_mut()) = new_length as MIDITick;
                }
            },
            EditorAction::VelocityChange(note_ids, delta_velocity, track) => {
                let mut notes = self.notes.write().unwrap();
                let notes = &mut notes[*track as usize];

                for (i, &id) in note_ids.iter().enumerate() {
                    let note = &mut notes[id];
                    let dt_velocity = &mut delta_velocity[i];
                    let mut new_velocity = (note.velocity() as i8).saturating_add(*dt_velocity);
                    if new_velocity < 1 { new_velocity = 1; }

                    *dt_velocity = new_velocity - note.velocity() as i8;
                    *(notes[id].velocity_mut()) = new_velocity as u8;
                }
            }
            /*EditorAction::Select(note_ids, track) => {

            },
            EditorAction::Deselect(note_ids, track) => {

            },*/
            EditorAction::Bulk(actions) => {
                for action in actions.iter_mut().rev() {
                    self.apply_action(action);
                }
            },
            _ => {}
        }
    }

    // ======== FLAG HELPER FUNCTIONS ========

    #[inline(always)]
    pub fn set_flag(&mut self, flag: u16, value: bool) {
        self.flags = (self.flags & !flag) | ((-(value as i16) as u16) & flag);
    }

    #[inline(always)]
    pub fn get_flag(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    #[inline(always)]
    pub fn enable_flag(&mut self, flag: u16) {
        self.set_flag(flag, true);
    }

    #[inline(always)]
    pub fn disable_flag(&mut self, flag: u16) {
        self.flags &= !flag;
    }

    // ======== NAV HELPER FUNCTIONS ========
    pub fn get_current_track(&self) -> u16 {
        let nav = self.nav.lock().unwrap();
        nav.curr_track
    }

    pub fn update_toolbar_settings_from_note(&self, note: &Note) {
        let mut tbs = self.toolbar_settings.try_borrow_mut().unwrap();
        tbs.note_gate.set_value(note.length());
        tbs.note_velocity.set_value(note.velocity());
        tbs.note_channel.set_value(note.channel() + 1);
    }

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

    // ======== MISC ========

    fn get_tbs_values(&self) -> (u8, MIDITick, u8) {
        let tbs = self.toolbar_settings.try_borrow().unwrap();
        (tbs.note_channel.value() as u8 - 1, tbs.note_gate.value() as MIDITick, tbs.note_velocity.value() as u8)
    }

    fn snap_tick(&self, tick: SignedMIDITick) -> SignedMIDITick {
        let snap = self.get_min_snap_tick_length() as SignedMIDITick;
        if snap == 1 { return tick; }

        let half = snap / 2;
        if tick >= 0 {
            ((tick + half) / snap) * snap
        } else {
            ((tick - half) / snap) * snap
        }
    }

    fn get_min_snap_tick_length(&self) -> MIDITick {
        let editor_tool = self.editor_tool.try_borrow().unwrap();
        let snap_ratio = editor_tool.snap_ratio;
        if snap_ratio.0 == 0 { return 1; }
        return (self.ppq as MIDITick * 4 * snap_ratio.0 as MIDITick)
            / snap_ratio.1 as MIDITick;
    }

    fn update_render_selected_notes(&self) {
        let mut render_manger = self.render_manager.lock().unwrap();
        render_manger.get_active_renderer().lock().unwrap().set_selected(&self.selected_note_ids);
    }

    pub fn get_can_draw_selection_box(&self) -> bool {
        self.draw_select_box
    }

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

    fn midi_pos_to_ui_pos(&self, ui: &mut Ui, tick_pos: MIDITick, key_pos: u8) -> (f32, f32) {
        let nav = self.nav.lock().unwrap();
        let rect = ui.min_rect();
        let mut ui_x = (tick_pos as f32 - nav.tick_pos_smoothed) / nav.zoom_ticks_smoothed;
        let mut ui_y = (key_pos as f32 - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;

        ui_x = ui_x * rect.width() + rect.left();
        ui_y = (1.0 - ui_y) * rect.height() + rect.top();

        (ui_x, ui_y)
    }

    pub fn update_cursor(&self, ctx: &Context, _ui: &mut Ui) {
        if self.get_flag(NOTE_EDIT_MOUSE_OVER_UI) {
            ctx.set_cursor_icon(CursorIcon::Default);
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool()
        };

        let is_at_note_end = self.mouse_info.is_at_note_end;

        match editor_tool {
            EditorTool::Pencil => {
                if is_at_note_end {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                    return;
                }

                if self.get_flag(NOTE_EDIT_MOUSE_OVER_NOTE) {
                    ctx.set_cursor_icon(CursorIcon::Move);
                    return;
                }
            },
            EditorTool::Eraser => {

            },
            EditorTool::Selector => {
                if is_at_note_end {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                    return;
                }

                if self.get_flag(NOTE_EDIT_MOUSE_OVER_NOTE) {
                    ctx.set_cursor_icon(CursorIcon::Move);
                    return;
                }

                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
        }
    }

    pub fn has_notes_in_clipboard(&self) -> bool {
        !self.notes_clipboard.is_empty()
    }

    pub fn is_any_note_selected(&self) -> bool {
        let sel_ids = self.selected_note_ids.lock().unwrap();
        !sel_ids.is_empty()
    }
}