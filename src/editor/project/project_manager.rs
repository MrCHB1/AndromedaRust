use std::{cell::RefCell, rc::Rc, sync::{Arc, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::{editor::{editing::meta_editing::MetaEditing, midi_bar_cacher::BarCacher, project::{self, project_data::{ProjectData, ProjectInfo}, ProjectWriter}, settings::editor_settings::ESGeneralSettings, tempo_map::TempoMap}, midi::{events::{channel_event::ChannelEvent, meta_event::MetaEvent, note::Note}, midi_file::MIDIFile}};

#[derive(Default)]
pub struct ProjectManager {
    pub project_data: ProjectData,
    pub project_info: ProjectInfo,
    pub ppq_changed: bool,
}

impl ProjectManager {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn change_ppq(&mut self, new_ppq: u16) {
        let project_data =  &mut self.project_data;
        project_data.ppq = new_ppq;

        self.get_tempo_map_mut().rebuild_tempo_map();
        self.ppq_changed = true;
    }

    pub fn get_ppq(&self) -> u16 {
        self.project_data.ppq
    }

    pub fn import_from_midi_file(&mut self, path: String) {
        let mut midi_file = MIDIFile::new();
        
        midi_file.with_track_discarding(false)
            .open(&path)
            .unwrap();

        self.project_data.load_data_from_midi_file(&mut midi_file);
    }

    pub fn save_project(&self) -> std::io::Result<()> {
        let save_dialog = rfd::FileDialog::new()
            .add_filter("Andromeda Project File", &["ama"]);
        
        if let Some(save_path) = save_dialog.save_file() {
            let pm = &self;
            let mut project_writer = ProjectWriter::new(pm, save_path);
            project_writer.write_header()?;
            project_writer.finalize()?;

            println!("project saved!");
        }

        Ok(())
    }

    pub fn new_empty_project(&mut self) {
        self.project_data.reset_or_init_data();
        let project_info = &mut self.project_info;

        project_info.name = "".into();
        project_info.author = "".into();
        project_info.description = "".into();
        self.project_data.ppq = 960;

        println!("Project has no notes? {}", self.is_project_empty(true));
    }

    pub fn get_tempo_map(&self) -> &Arc<RwLock<TempoMap>> {
        &self.project_data.tempo_map
    }

    pub fn get_tempo_map_mut(&self) -> RwLockWriteGuard<'_, TempoMap> {
        self.project_data.tempo_map.write().unwrap()
    }

    pub fn get_project_data(&self) -> &ProjectData {
        &self.project_data
    }

    pub fn get_project_data_mut(&mut self) -> &mut ProjectData {
        &mut self.project_data
    }

    pub fn get_project_info(&self) -> &ProjectInfo {
        &self.project_info
    }

    pub fn get_project_info_mut(&mut self) -> &mut ProjectInfo {
        &mut self.project_info
    }

    pub fn get_metas(&self) -> &Arc<RwLock<Vec<MetaEvent>>> {
        &self.project_data.global_metas
    }

    pub fn get_notes(&self) -> &Arc<RwLock<Vec<Vec<Note>>>> {
        &self.project_data.notes
    }

    pub fn get_channel_evs(&self) -> &Arc<RwLock<Vec<Vec<ChannelEvent>>>> {
        &self.project_data.channel_events
    }

    pub fn is_project_empty(&self, notes_only: bool) -> bool {
        let notes = self.get_notes().read().unwrap();
        let mut empty = notes.is_empty();
        if notes_only { return empty; }
        
        let ch_evs = self.get_channel_evs().read().unwrap();
        empty = empty && ch_evs.is_empty();
        empty
    }
}