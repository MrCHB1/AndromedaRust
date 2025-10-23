#![warn(unused)]
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::{Arc, Mutex, RwLock}};

use eframe::egui::{self, Ui};

use crate::{app::{main_window::{EditorTool, EditorToolSettings}, rendering::note_cull_helper::NoteCullHelper, view_settings::ViewSettings}, editor::{actions::{EditorAction, EditorActions}, navigation::{PianoRollNavigation, TrackViewNavigation}, project::{project_data::ProjectData, project_manager::ProjectManager}, util::{get_mouse_track_view_pos, MIDITick}}, midi::events::{channel_event::ChannelEvent, note::Note}};

pub mod track_flags {
    pub const TRACK_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const TRACK_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const TRACK_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x2;
    pub const TRACK_EDIT_ANY_DIALOG_OPEN: u16 = 0x4;
}

use track_flags::*;

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

    curr_mouse_track_pos: (MIDITick, u16),
    right_clicked_track: u16,
    flags: u16,
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

            curr_mouse_track_pos: (0, 0),
            view_settings: view_settings.clone(),
            flags: TRACK_EDIT_FLAGS_NONE,
            right_clicked_track: 0,
            ppq: 960,
        }
    }

    #[inline(always)]
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
    }

    pub fn insert_track(&mut self, track: u16) {
        let track_count = self.get_used_track_count();
        if track >= track_count { return; }

        self.insert_notes_and_ch_evs(track, Vec::new(), Vec::new());

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::AddTrack(track, None, false));
    }

    fn insert_notes_and_ch_evs(&mut self, track: u16, notes: Vec<Note>, ch_evs: Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        let (mut notes_, mut ch_evs_) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );

        notes_.insert(track as usize, notes);
        ch_evs_.insert(track as usize, ch_evs);
    }

    pub fn remove_track(&mut self, track: u16) {
        let mut track_count = self.get_used_track_count();
        if track >= track_count { return; }

        {
            let (removed_track_notes, removed_track_ch_evs) = self.remove_note_and_ch_evs(track);
            track_count -= 1;

            let mut removed_first = false;
            if track_count == 0 {
                // always guarantee at least one track
                // self.append_empty_track();
                track_count = 1;
                removed_first = true;
            }
            
            let mut removed_track_queue = VecDeque::new();
            removed_track_queue.push_back((removed_track_notes, removed_track_ch_evs)); 

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

    pub fn remove_note_and_ch_evs(&mut self, track: u16) -> (Vec<Note>, Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        let (mut notes, mut ch_evs) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );

        (notes.remove(track as usize), ch_evs.remove(track as usize))
    }

    pub fn append_empty_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let (mut notes, mut ch_evs) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );

        notes.push(Vec::new());
        ch_evs.push(Vec::new());
    }

    pub fn pop_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let (mut notes, mut ch_evs) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );

        notes.pop();
        ch_evs.pop();
    }

    pub fn get_right_clicked_track(&self) -> u16 {
        self.right_clicked_track
    }

    pub fn remove_right_clicked_track(&mut self) {
        self.remove_track(self.right_clicked_track);
    }

    pub fn get_used_track_count(&self) -> u16 {
        let project_manager = self.project_manager.read().unwrap();
        let notes = project_manager.get_notes().read().unwrap();
        notes.len() as u16
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

            let (mut notes, mut ch_evs) = (
                project_data.notes.write().unwrap(),
                project_data.channel_events.write().unwrap()
            );

            notes.swap(track_1 as usize, track_2 as usize);
            ch_evs.swap(track_1 as usize, track_2 as usize);
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

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::AddTrack(track_insert, removed_tracks, is_first) => {
                assert!(removed_tracks.is_some(), "[ADD_TRACKS] Something has gone wrong while trying to add a track.");

                let mut recovered_track = removed_tracks.take().unwrap();
                let recovered_track = recovered_track.pop_front().unwrap();
                self.insert_notes_and_ch_evs(*track_insert, recovered_track.0, recovered_track.1);

                // change current track if inserted before current track
                let curr_track = self.get_pianoroll_track();
                if *track_insert <= curr_track {
                    self.change_track(curr_track + 1);
                }
            },
            EditorAction::RemoveTrack(track_rem_idx, removed_tracks, is_first) => {
                let mut rem_track_queue = VecDeque::with_capacity(1);

                let removed_track = self.remove_note_and_ch_evs(*track_rem_idx);
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
}