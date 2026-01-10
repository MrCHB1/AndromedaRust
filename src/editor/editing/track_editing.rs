#![warn(unused)]
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::{Arc, Mutex, RwLock}};

use eframe::egui::{self, Ui};

use crate::{app::{main_window::{EditorTool, EditorToolSettings}, rendering::note_cull_helper::NoteCullHelper, view_settings::ViewSettings}, editor::{actions::{EditorAction, EditorActions}, navigation::{PianoRollNavigation, TrackViewNavigation}, project::{project_data::ProjectData, project_manager::ProjectManager}, util::{MIDITick, SignedMIDITick, get_mouse_track_view_pos}}, midi::{events::{channel_event::ChannelEvent, note::Note}, midi_track::MIDITrack}};

pub mod track_flags {
    pub const TRACK_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const TRACK_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const TRACK_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x2;
    pub const TRACK_EDIT_ANY_DIALOG_OPEN: u16 = 0x4;
}

use track_flags::*;

#[derive(Default)]
struct TrackEditMouseInfo {
    mouse_midi_track_pos: (MIDITick, u16),
    last_mouse_click_pos: (MIDITick, u16),
}

// because im a lazy mf, i put note track editing as a separate file
#[derive(Default)]
pub struct TrackEditing {
    project_manager: Arc<RwLock<ProjectManager>>,
    // project_manager: Arc<RwLock<ProjectManager>>,
    view_settings: Arc<Mutex<ViewSettings>>,

    editor_tool: Rc<RefCell<EditorToolSettings>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    pr_nav: Arc<Mutex<PianoRollNavigation>>,
    nav: Arc<Mutex<TrackViewNavigation>>,
    mouse_info: TrackEditMouseInfo,

    curr_mouse_track_pos: (MIDITick, u16),
    right_clicked_track: u16,
    flags: u16,
    
    pub selection_range: (MIDITick, MIDITick, u16, u16),
    draw_select_box: bool,

    pub ppq: u16,
}

impl TrackEditing {
    pub fn new(
        project_manager: &Arc<RwLock<ProjectManager>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        editor_actions: &Rc<RefCell<EditorActions>>,

        pr_nav: &Arc<Mutex<PianoRollNavigation>>,
        nav: &Arc<Mutex<TrackViewNavigation>>,

        view_settings: &Arc<Mutex<ViewSettings>>,
    ) -> Self {
        Self {
            project_manager: project_manager.clone(),
            // ppq,
            editor_tool: editor_tool.clone(),
            editor_actions: editor_actions.clone(),

            pr_nav: pr_nav.clone(),
            nav: nav.clone(),
            mouse_info: Default::default(),

            curr_mouse_track_pos: (0, 0),
            view_settings: view_settings.clone(),
            flags: TRACK_EDIT_FLAGS_NONE,
            right_clicked_track: 0,
            ppq: 960,
            selection_range: (0, 0, 0, 0),
            draw_select_box: false
        }
    }

    pub fn update(&mut self, ui: &mut Ui) {
        // convert from mouse pos to midi track pos
        let (mouse_x, mouse_y) = {
            let pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
            (pos.x, pos.y)
        };

        // self.mouse_info.mouse_midi_track_pos = get_mouse_track_view_pos(ui, &self.nav);
        self.mouse_info.mouse_midi_track_pos = self.screen_pos_to_midi_track_pos((mouse_x, mouse_y), ui);
    }

    fn screen_pos_to_midi_track_pos(&self, screen_pos: (f32, f32), ui: &mut Ui) -> (MIDITick, u16) {
        let rect = ui.min_rect();
        
        let screen_x_norm = (screen_pos.0 - rect.left()) / rect.width();
        let screen_y_norm = (screen_pos.1 - rect.top()) / rect.height();

        let nav = self.nav.lock().unwrap();
        let screen_x_tick = (screen_x_norm * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as MIDITick;
        let screen_y_trck = (screen_y_norm * nav.zoom_tracks_smoothed + nav.track_pos_smoothed) as u16;

        (screen_x_tick, screen_y_trck)
    }

    fn midi_track_pos_to_screen_pos(&self, midi_track_pos: (MIDITick, u16), ui: &mut Ui) -> (f32, f32) {
        let rect = ui.min_rect();
        let nav = self.nav.lock().unwrap();

        let screen_x_norm = (midi_track_pos.0 as f32 - nav.tick_pos_smoothed) / nav.zoom_ticks_smoothed;
        let screen_y_norm = (midi_track_pos.1 as f32 - nav.track_pos_smoothed) / nav.zoom_tracks_smoothed;

        let screen_x = rect.left() + screen_x_norm * rect.width();
        let screen_y = rect.top() + screen_y_norm * rect.height();

        (screen_x, screen_y)
    }

    // ======== INPUT EVENTS ========
    pub fn on_mouse_down(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI) {
            self.enable_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {
                let track_pos = self.get_mouse_track_pos();
                self.change_track(track_pos);
            },
            EditorTool::Selector => {
                self.select_mouse_down();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI) { return; }

        self.right_clicked_track = self.get_mouse_track_pos();
    }

    pub fn on_mouse_move(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI) { return; }
        if self.get_flag(TRACK_EDIT_ANY_DIALOG_OPEN | TRACK_EDIT_MOUSE_OVER_UI) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {

            },
            EditorTool::Selector => {
                self.select_mouse_move();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_mouse_up(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI) {
            self.disable_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI | TRACK_EDIT_ANY_DIALOG_OPEN) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {

            },
            EditorTool::Selector => {
                self.select_mouse_up();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_key_down(&mut self, ui: &mut Ui) {
        let curr_track = self.get_pianoroll_track();

        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.command) {
            if curr_track > 0 { self.change_track(curr_track - 1); }
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.command) {
            if curr_track < u16::MAX { self.change_track(curr_track + 1); }
        }
    }

    // ======== TOOL MOUSE EVENTS ========

    fn select_mouse_down(&mut self) {
        self.init_selection_box(self.mouse_info.mouse_midi_track_pos);
    }

    fn select_mouse_move(&mut self) {
        self.update_selection_box(self.mouse_info.mouse_midi_track_pos);
    }

    fn select_mouse_up(&mut self) {
        self.draw_select_box = false;

        let (min_tick, max_tick, min_track, max_track) = self.get_selection_range();

    }

    // ======== SELECTION BOX ========

    fn init_selection_box(&mut self, start_pos: (MIDITick, u16)) {
        let snapped_tick = self.snap_tick(start_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range = (snapped_tick, snapped_tick, start_pos.1, start_pos.1);
        self.draw_select_box = true;
    }

    fn update_selection_box(&mut self, new_pos: (MIDITick, u16)) {
        self.selection_range.1 = self.snap_tick(new_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range.3 = new_pos.1;
    }

    fn get_selection_range(&self) -> (MIDITick, MIDITick, u16, u16) {
        let (min_tick, max_tick) = {
            if self.selection_range.0 > self.selection_range.1 {
                (self.selection_range.1, self.selection_range.0)
            } else {
                (self.selection_range.0, self.selection_range.1)
            }
        };

        let (min_track, max_track) = {
            if self.selection_range.2 > self.selection_range.3 {
                (self.selection_range.3, self.selection_range.2)
            } else {
                (self.selection_range.2, self.selection_range.3)
            }
        };

        (min_tick, max_tick, min_track, max_track)
    }

    #[inline(always)]
    pub fn get_can_draw_selection_box(&self) -> bool {
        self.draw_select_box
    }

    pub fn get_selection_range_ui(&self, ui: &mut Ui) -> ((f32, f32), (f32, f32)) {
        let (min_tick, max_tick, min_track, max_track) = self.get_selection_range();

        let tl = self.midi_track_pos_to_screen_pos((min_tick, min_track), ui);
        let br = self.midi_track_pos_to_screen_pos((max_tick, max_track), ui);

        (tl, br)
    }

    // ======== HELPER FUNCTIONS ========

    #[inline(always)]
    pub fn get_mouse_track_pos(&self) -> u16 {
        self.mouse_info.mouse_midi_track_pos.1
        // self.curr_mouse_track_pos.1
    }

    fn get_pianoroll_track(&self) -> u16 {
        let nav = self.pr_nav.lock().unwrap();
        nav.curr_track
    }

    fn insert_notes_and_ch_evs(&mut self, track: u16, notes: Vec<Note>, ch_evs: Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut track_ = project_manager.get_tracks().write().unwrap();
        /*let (mut notes_, mut ch_evs_) = (
            let mut track = 
            
            // project_manager.get_notes().write().unwrap(),
            // project_manager.get_channel_evs().write().unwrap()
        );*/

        track_.insert(track as usize, MIDITrack::new(notes, ch_evs, Vec::new()));
        // notes_.insert(track as usize, notes);
        // ch_evs_.insert(track as usize, ch_evs);
    }

    fn insert_track_at(&mut self, track_idx: u16, track: MIDITrack) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.insert(track_idx as usize, track);
    }

    pub fn insert_track(&mut self, track: u16) {
        let track_count = self.get_used_track_count();
        if track >= track_count { return; }

        self.insert_notes_and_ch_evs(track, Vec::new(), Vec::new());

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::AddTrack(track, None, false));
    }

    pub fn remove_track(&mut self, track: u16) {
        let mut track_count = self.get_used_track_count();
        if track >= track_count { return; }

        {
            // let (removed_track_notes, removed_track_ch_evs) = self.remove_note_and_ch_evs(track);
            let removed_track = self.remove_track_at(track);
            track_count -= 1;

            let mut removed_first = false;
            if track_count == 0 {
                // always guarantee at least one track
                // self.append_empty_track();
                track_count = 1;
                removed_first = true;
            }
            
            let mut removed_track_queue = VecDeque::new();
            removed_track_queue.push_back(removed_track);
            // removed_track_queue.push_back((removed_track_notes, removed_track_ch_evs)); 

            let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
            editor_actions.register_action(EditorAction::RemoveTrack(track, Some(removed_track_queue), removed_first));
        }

        {
            let pr_track = self.get_pianoroll_track();
            if pr_track > track {
                self.change_track(pr_track - 1);
            }
        }
    }

    pub fn remove_track_at(&mut self, track: u16) -> MIDITrack {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.remove(track as usize)
    }

    /*pub fn remove_note_and_ch_evs(&mut self, track: u16) -> (Vec<Note>, Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut track_ = project_manager.get_tracks().write().unwrap();
        /*let (mut notes, mut ch_evs) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );*/

        (notes.remove(track as usize), ch_evs.remove(track as usize))
    }*/

    pub fn append_empty_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.push(MIDITrack::new_empty());
    }

    pub fn pop_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.pop();
    }

    pub fn get_right_clicked_track(&self) -> u16 {
        self.right_clicked_track
    }

    pub fn remove_right_clicked_track(&mut self) {
        self.remove_track(self.right_clicked_track);
    }

    pub fn get_used_track_count(&self) -> u16 {
        let project_manager = self.project_manager.read().unwrap();
        let tracks = project_manager.get_tracks().read().unwrap();
        tracks.len() as u16
        // let notes = project_manager.get_notes().read().unwrap();
        // notes.len() as u16
    }

    pub fn change_track(&mut self, new_track: u16) {
        {
            let mut view_settings = self.view_settings.lock().unwrap();
            view_settings.pr_curr_track.set_value(new_track);
        }

        {
            //let mut project_data = self.project_data.try_borrow_mut().unwrap();
            let mut project_manager = self.project_manager.write().unwrap();
            project_manager.get_project_data_mut().validate_tracks(new_track);
        }

        {
            let mut nav = self.pr_nav.lock().unwrap();
            nav.curr_track = new_track;
        }
    }

    fn swap_tracks_and_register(&mut self, track_1: u16, track_2: u16, allow_register: bool) {
        let track_count = self.get_used_track_count();

        {
            let mut project_manager = self.project_manager.write().unwrap();
            let project_data = project_manager.get_project_data_mut();
            
            if track_1 >= track_count || track_2 >= track_count {
                project_data.validate_tracks(track_1.max(track_2));
            }

            let mut tracks = project_data.tracks.write().unwrap();
            tracks.swap(track_1 as usize, track_2 as usize);
        }

        if allow_register {
            let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
            editor_actions.register_action(EditorAction::SwapTracks(track_1, track_2));
        }

        let curr_track = self.get_pianoroll_track();
        if curr_track == track_1 { self.change_track(track_2); return; }
        if curr_track == track_2 { self.change_track(track_1); return; }
    }

    pub fn swap_tracks(&mut self, track_1: u16, track_2: u16) {
        self.swap_tracks_and_register(track_1, track_2, true);
    }

    // ======== ACTIONS ========

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::AddTrack(track_insert, removed_tracks, is_first) => {
                assert!(removed_tracks.is_some(), "[ADD_TRACKS] Something has gone wrong while trying to add a track.");

                let mut recovered_track = removed_tracks.take().unwrap();
                let recovered_track = recovered_track.pop_front().unwrap();
                self.insert_track_at(*track_insert, recovered_track);

                // change current track if inserted before current track
                let curr_track = self.get_pianoroll_track();
                if *track_insert <= curr_track {
                    self.change_track(curr_track + 1);
                }
            },
            EditorAction::RemoveTrack(track_rem_idx, removed_tracks, is_first) => {
                let mut rem_track_queue = VecDeque::with_capacity(1);

                let removed_track = self.remove_track_at(*track_rem_idx);
                rem_track_queue.push_front(removed_track);

                *removed_tracks = Some(rem_track_queue);

                // change current track if removed before current track
                let curr_track = self.get_pianoroll_track();
                if *track_rem_idx < curr_track {
                    self.change_track(curr_track - 1);
                }
            },
            EditorAction::SwapTracks(track_1, track_2) => {
                self.swap_tracks_and_register(*track_1, *track_2, false);
            },
            _ => {}
        }
    }

    // ======== MISC ========

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
            /  snap_ratio.1 as MIDITick;
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

    /*#[inline(always)]
    pub fn set_mouse_over_ui(&mut self, mouse_over_ui: bool) {
        self.flags &= !TRACK_EDIT_MOUSE_OVER_UI;
        if mouse_over_ui { self.flags |= TRACK_EDIT_MOUSE_OVER_UI; }
    }

    pub fn update_from_ui(&mut self, ui: &mut Ui) {
        let mouse_track_pos = get_mouse_track_view_pos(ui, &self.nav);
        self.curr_mouse_track_pos = mouse_track_pos;
    }

    #[inline(always)]
    pub fn get_mouse_track_pos(&self) -> u16 {
        self.curr_mouse_track_pos.1
    }

    // helper functions
    fn get_pianoroll_track(&self) -> u16 {
        let nav = self.pr_nav.lock().unwrap();
        nav.curr_track
    }

    pub fn on_mouse_down(&mut self) {
        if self.flags & TRACK_EDIT_MOUSE_OVER_UI != 0 {
            self.flags |= TRACK_EDIT_MOUSE_DOWN_ON_UI;
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {
                let track_pos = self.get_mouse_track_pos();
                self.change_track(track_pos);
            },
            EditorTool::Selector => {

            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.flags & TRACK_EDIT_MOUSE_OVER_UI != 0 {
            return;
        }

        self.right_clicked_track = self.get_mouse_track_pos();
    }

    pub fn on_mouse_move(&mut self) {

    }

    pub fn on_mouse_up(&mut self) {

    }

    pub fn on_key_down(&mut self, ui: &mut Ui) {
        let curr_track = self.get_pianoroll_track();

        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.command) {
            if curr_track > 0 { self.change_track(curr_track - 1); }
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.command) {
            if curr_track < u16::MAX { self.change_track(curr_track + 1); }
        }
    }*/
}