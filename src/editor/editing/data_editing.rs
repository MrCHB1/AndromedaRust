use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, RwLock}};

use crate::{app::{main_window::{EditorTool, EditorToolSettings}, view_settings::{VS_PianoRoll_DataViewState, ViewSettings}}, editor::{navigation::PianoRollNavigation, util::{bin_search_notes, MIDITick}}, midi::events::{channel_event::ChannelEvent, note::Note}};

pub mod data_edit_flags {
    pub const DATA_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const DATA_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const DATA_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x2;
    pub const DATA_EDIT_ANY_DIALOG_OPEN: u16 = 0x4;
    pub const DATA_EDIT_DRAW_EDIT_LINE: u16 = 0x8;
}

use data_edit_flags::*;
use eframe::egui::Ui;

type DataNumType = i16;

#[derive(Default)]
pub struct DataEditMouseInfo {
    pub mouse_data_pos: (MIDITick, DataNumType),
    pub mouse_screen_pos: (f32, f32),
    pub last_data_click_pos: (MIDITick, DataNumType),
    pub last_screen_click_pos: (f32, f32),
}

/// Handles editing stuff like Note Velocities, pitch bends, tempo, etc.
#[derive(Default)]
pub struct DataEditing {
    notes: Arc<RwLock<Vec<Vec<Note>>>>,
    channel_events: Arc<RwLock<Vec<Vec<ChannelEvent>>>>,
    view_settings: Arc<Mutex<ViewSettings>>,

    nav: Arc<Mutex<PianoRollNavigation>>,

    editor_tool: Rc<RefCell<EditorToolSettings>>,
    mouse_info: DataEditMouseInfo,
    flags: u16,
}

impl DataEditing {
    pub fn new(
        notes: &Arc<RwLock<Vec<Vec<Note>>>>,
        channel_events: &Arc<RwLock<Vec<Vec<ChannelEvent>>>>,
        view_settings: &Arc<Mutex<ViewSettings>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        nav: &Arc<Mutex<PianoRollNavigation>>
    ) -> Self {
        Self {
            notes: notes.clone(),
            channel_events: channel_events.clone(),
            view_settings: view_settings.clone(),

            nav: nav.clone(),

            editor_tool: editor_tool.clone(),
            mouse_info: Default::default(),
            flags: DATA_EDIT_FLAGS_NONE
        }
    }

    // ======== MOUSE EVENT FUNCTIONS ========

    pub fn on_mouse_down(&mut self) {
        if self.get_flag(DATA_EDIT_MOUSE_OVER_UI) {
            self.enable_flag(DATA_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(DATA_EDIT_ANY_DIALOG_OPEN) { return; }


        let editor_tool = {
            let editor_tool = self.editor_tool.borrow();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => self.pencil_mouse_down(),
            EditorTool::Eraser => self.eraser_mouse_down(),
            EditorTool::Selector => self.select_mouse_down()
        }
    }

    pub fn on_mouse_move(&mut self) {
        if self.get_flag(DATA_EDIT_MOUSE_DOWN_ON_UI) { return; }

        if self.get_flag(DATA_EDIT_ANY_DIALOG_OPEN | DATA_EDIT_MOUSE_OVER_UI) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.borrow();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => self.pencil_mouse_move(),
            EditorTool::Eraser => self.eraser_mouse_move(),
            EditorTool::Selector => self.select_mouse_move()
        }
    }

    pub fn on_mouse_up(&mut self) {
        if self.get_flag(DATA_EDIT_MOUSE_DOWN_ON_UI) { 
            self.disable_flag(DATA_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(DATA_EDIT_MOUSE_OVER_UI | DATA_EDIT_ANY_DIALOG_OPEN) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.borrow();
            editor_tool.get_tool()
        };

        match editor_tool {
            EditorTool::Pencil => self.pencil_mouse_up(),
            EditorTool::Eraser => self.eraser_mouse_up(),
            EditorTool::Selector => self.select_mouse_up()
        }
    }

    // ======== HELPER FUNCS ========
    pub fn update(&mut self, ui: &mut Ui) {
        // convert from mouse pos to data view pos
        let (mouse_x, mouse_y) = {
            let pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
            (pos.x, pos.y)
        };

        self.mouse_info.mouse_data_pos = self.screen_pos_to_data_pos((mouse_x, mouse_y), ui);
        self.mouse_info.mouse_screen_pos = (mouse_x, mouse_y);
    }

    fn screen_pos_to_data_pos(&self, screen_pos: (f32, f32), ui: &mut Ui) -> (MIDITick, DataNumType) {
        let rect = ui.min_rect();
        let screen_x_norm = (screen_pos.0 - rect.left()) / rect.width();
        let screen_y_norm = 1.0 - (screen_pos.1 - rect.top() - 21.0) / 200.0; // ugly hardcoded values :skull:
        
        let nav = self.nav.lock().unwrap();
        let screen_x_tick = (screen_x_norm * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as MIDITick;
        let screen_y_data = self.scaled_y_from_curr_data(screen_y_norm); // TODO: make y pos scaled based on current data
        drop(nav);

        (screen_x_tick, screen_y_data)
    }

    fn data_pos_to_screen_pos(&self, data_pos: (MIDITick, DataNumType), ui: &mut Ui) -> (f32, f32) {
        let nav = self.nav.lock().unwrap();
        let data_x_norm = (data_pos.0 as f32 - nav.tick_pos_smoothed) / nav.zoom_keys_smoothed;
        let data_y_norm = self.unscaled_y_from_curr_data(data_pos.1);
        drop(nav);

        let rect = ui.min_rect();
        let data_x_scr = data_x_norm * rect.width() + rect.left();
        let data_y_scr = (1.0 - data_y_norm) * 200.0 + 21.0 + rect.top();

        (data_x_scr, data_y_scr)
    }

    fn scaled_y_from_curr_data(&self, y: f32) -> DataNumType {
        let vs = self.view_settings.lock().unwrap();
        match vs.pr_dataview_state {
            VS_PianoRoll_DataViewState::NoteVelocities => {
                ((y * 127.0) as DataNumType).clamp(0, 127)
            },
            VS_PianoRoll_DataViewState::PitchBend => {
                (((y * 2.0 - 1.0) * 8192.0) as DataNumType).clamp(-8192, 8191)
            },
            _ => 0
        }
    }

    fn unscaled_y_from_curr_data(&self, y: DataNumType) -> f32 {
        let vs = self.view_settings.lock().unwrap();
        match vs.pr_dataview_state {
            VS_PianoRoll_DataViewState::NoteVelocities => {
                y as f32 / 127.0
            },
            VS_PianoRoll_DataViewState::PitchBend => {
                y as f32 / 8192.0 * 0.5 + 0.5
            },
            _ => 0.0
        }
    }

    // ======== TOOL MOUSE EVENT FUNCTIONS ========

    // PENCIL EVENTS
    fn pencil_mouse_down(&mut self) {
        // snapshot the current mouse pos in data view space
        self.update_last_mouse_data_pos();
        self.enable_flag(DATA_EDIT_DRAW_EDIT_LINE);
    }

    fn pencil_mouse_move(&mut self) {

    }

    fn pencil_mouse_up(&mut self) {
        self.disable_flag(DATA_EDIT_DRAW_EDIT_LINE);

        // get points in data space
        let (data_pos_1, data_pos_2) = {
            let mouse_info = &self.mouse_info;
            let (dp1, dp2) = (mouse_info.last_data_click_pos, mouse_info.mouse_data_pos);
            if dp1.0 > dp2.0 { (dp2, dp1) }
            else { (dp1, dp2) }
        };

        let dataview_state = {
            let vs = self.view_settings.lock().unwrap();
            vs.pr_dataview_state
        };

        match dataview_state {
            VS_PianoRoll_DataViewState::NoteVelocities => {
                self.set_note_velocities_ranged(data_pos_1.0, data_pos_1.1 as u8, data_pos_2.0, data_pos_2.1 as u8);
            },
            VS_PianoRoll_DataViewState::PitchBend => {

            },
            VS_PianoRoll_DataViewState::Hidden => {

            }
        }
    }

    // ERASER EVENTS
    fn eraser_mouse_down(&mut self) {

    }

    fn eraser_mouse_move(&mut self) {

    }

    fn eraser_mouse_up(&mut self) {

    }

    // SELECTOR EVENTS
    fn select_mouse_down(&mut self) {

    }

    fn select_mouse_move(&mut self) {

    }

    fn select_mouse_up(&mut self) {
        
    }

    // ======== DATA EDITING ========
    fn set_note_velocities_ranged(&mut self, min_tick: MIDITick, min_velocity: u8, max_tick: MIDITick, max_velocity: u8) {
        let curr_track = self.get_curr_track();
        
        let mut notes = self.notes.write().unwrap();
        let notes = &mut notes[curr_track as usize];

        let note_id_min = match notes.binary_search_by_key(&min_tick, |&n| n.start()) {
            Ok(id) | Err(id) => id
        };

        let note_id_max = match notes.binary_search_by_key(&max_tick, |&n| n.start()) {
            Ok(id) | Err(id) => id
        };

        for note in notes[note_id_min..note_id_max].iter_mut() {
            let note_tick = note.start();
            let vel_factor = (note_tick - min_tick) as f32 / (max_tick - min_tick) as f32;
            let vel_mix = ((1.0 - vel_factor) * min_velocity as f32 + vel_factor * max_velocity as f32) as u8;
            *(note.velocity_mut()) = vel_mix;
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

    // ======== OTHER FUNCTIONS ========
    fn update_last_mouse_data_pos(&mut self) {
        let mouse_info = &mut self.mouse_info;
        mouse_info.last_data_click_pos = mouse_info.mouse_data_pos;
        mouse_info.last_screen_click_pos = mouse_info.mouse_screen_pos;
    }

    pub fn get_data_view_line_points(&self) -> ((f32, f32), (f32, f32)) {
        let point_1 = self.mouse_info.last_screen_click_pos;
        let point_2 = self.mouse_info.mouse_screen_pos;

        if point_1.0 > point_2.0 {
            (point_2, point_1)
        } else {
            (point_1, point_2)
        }
        //(point_1, point_2)
    }

    fn get_curr_track(&self) -> u16 {
        let nav = self.nav.lock().unwrap();
        nav.curr_track
    }
}