use std::sync::{Arc, Mutex, RwLock};

use crate::{app::main_window::EditorToolSettings, editor::actions::EditorActions, midi::events::{channel_event::ChannelEvent, note::Note}};

// because im a lazy mf, i put note track editing as a separate file
#[derive(Default)]
pub struct TrackEditing {
    notes: Arc<RwLock<Vec<Vec<Note>>>>,
    ch_evs: Arc<RwLock<Vec<Vec<ChannelEvent>>>>,

    editor_tool: Arc<Mutex<EditorToolSettings>>,
    editor_actions: Arc<Mutex<EditorActions>>,
    ppq: u16
}

impl TrackEditing {
    pub fn new(
        notes: &Arc<RwLock<Vec<Vec<Note>>>>,
        ch_evs: &Arc<RwLock<Vec<Vec<ChannelEvent>>>>,
    ) -> () {

    }
}