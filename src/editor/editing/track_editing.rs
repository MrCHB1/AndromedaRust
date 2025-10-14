#![warn(unused)]
use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, RwLock}};

use eframe::egui::Ui;

use crate::{app::main_window::EditorToolSettings, editor::{actions::EditorActions, navigation::{PianoRollNavigation, TrackViewNavigation}, project_data::ProjectData, util::{get_mouse_track_view_pos, MIDITick}}, midi::events::{channel_event::ChannelEvent, note::Note}};

// because im a lazy mf, i put note track editing as a separate file
#[derive(Default)]
pub struct TrackEditing {
    notes: Arc<RwLock<Vec<Vec<Note>>>>,
    ch_evs: Arc<RwLock<Vec<Vec<ChannelEvent>>>>,

    editor_tool: Rc<RefCell<EditorToolSettings>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    pr_nav: Arc<Mutex<PianoRollNavigation>>,
    nav: Arc<Mutex<TrackViewNavigation>>,
    ppq: u16,

    curr_mouse_track_pos: (MIDITick, u16)
}

impl TrackEditing {
    pub fn new(
        project_data: &Rc<RefCell<ProjectData>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        editor_actions: &Rc<RefCell<EditorActions>>,

        pr_nav: &Arc<Mutex<PianoRollNavigation>>,
        nav: &Arc<Mutex<TrackViewNavigation>>
    ) -> Self {
        let (notes, ch_evs, ppq) = {
            let project_data = project_data.borrow();
            let notes = project_data.notes.clone();
            let ch_evs = project_data.channel_events.clone();
            let ppq = project_data.project_info.ppq;
            (notes, ch_evs, ppq)
        };

        Self {
            notes,
            ch_evs,
            ppq,
            editor_tool: editor_tool.clone(),
            editor_actions: editor_actions.clone(),

            pr_nav: pr_nav.clone(),
            nav: nav.clone(),

            curr_mouse_track_pos: (0, 0)
        }
    }

    pub fn update_from_ui(&mut self, ui: &mut Ui) {
        let mouse_track_pos = get_mouse_track_view_pos(ui, &self.nav);

        self.curr_mouse_track_pos = mouse_track_pos;
    }

    pub fn on_mouse_down(&mut self) {
        println!("{:?}", self.curr_mouse_track_pos);
    }

    pub fn on_mouse_move(&mut self) {

    }
}