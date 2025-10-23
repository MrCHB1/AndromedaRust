// houses all information such as notes, tempos, control/meta events, etc.
// kinda like midi_file.rs but editable lol

use crate::{editor::{tempo_map::TempoMap, util::tempo_as_bytes}, midi::{events::{channel_event::ChannelEvent, meta_event::{MetaEvent, MetaEventType}, note::Note}, midi_file::{self, MIDIFile}}};
use std::sync::{Arc, RwLock};

pub struct ProjectInfo {
    pub name: String,
    pub author: String,
    pub description: String,
    // pub ppq: u16
}

impl Default for ProjectInfo {
    fn default() -> Self {
        Self {
            name: "".into(),
            author: "".into(),
            description: "".into(),
            // ppq: 960
        }
    }
}

#[derive(Default)]
pub struct ProjectData {
    pub ppq: u16,
    // pub project_info: ProjectInfo,
    // 16 channels per track. each channel contains vector of notes
    pub notes: Arc<RwLock<Vec<Vec<Note>>>>,
    pub global_metas: Arc<RwLock<Vec<MetaEvent>>>,
    pub channel_events: Arc<RwLock<Vec<Vec<ChannelEvent>>>>,
    pub tempo_map: Arc<RwLock<TempoMap>>
}

impl ProjectData {
    /*pub fn import_from_midi_file(&mut self, path: String) {
        let project_info = &mut self.project_info;
        let mut file = midi_file::MIDIFile::new();
        
        file.with_track_discarding(false)
            .open(&path)
            .unwrap();
        self.ppq = file.ppq;

        //self.notes = Arc::new(Mutex::new(std::mem::take(&mut file.notes)));
        {
            file.preprocess_meta_events(); // will merge specific meta events into one track

            *(self.notes.write().unwrap()) = std::mem::take(&mut file.notes);
            *(self.global_metas.write().unwrap()) = std::mem::take(&mut file.global_meta_events);
            *(self.channel_events.write().unwrap()) = std::mem::take(&mut file.channel_events);
        }

        {
            let mut tempo_map = self.tempo_map.write().unwrap();
            // tempo_map.meta_events = self.global_metas.clone();
            tempo_map.rebuild_tempo_map();
        }
        
        project_info.name = "";
        project_info.author = "";
        project_info.description = "";
        // project_info.ppq = file.ppq;
    }*/

    pub fn load_data_from_midi_file(&mut self, midi_file: &mut MIDIFile) {
        self.ppq = midi_file.ppq;
        {
            midi_file.preprocess_meta_events();

            *(self.notes.write().unwrap()) = std::mem::take(&mut midi_file.notes);
            *(self.global_metas.write().unwrap()) = std::mem::take(&mut midi_file.global_meta_events);
            *(self.channel_events.write().unwrap()) = std::mem::take(&mut midi_file.channel_events);
        }

        {
            let mut tempo_map = self.tempo_map.write().unwrap();
            tempo_map.rebuild_tempo_map();
        }
    }

    pub fn reset_or_init_data(&mut self) {
        /* let project_info = &mut self.project_info;
        project_info.name = "";
        project_info.author = "";
        project_info.description = "";
        project_info.ppq = 960; */
        
        {
            let mut notes = self.notes.write().unwrap();
            let mut ch_evs = self.channel_events.write().unwrap();

            notes.clear();
            ch_evs.clear();

            // initialize an empty track
            notes.push(Vec::new());
            ch_evs.push(Vec::new());
        }

        // initialize default meta events
        {
            let mut global_metas = self.global_metas.write().unwrap();
            *global_metas = vec![
                MetaEvent {
                    tick: 0,
                    event_type: MetaEventType::Tempo,
                    data: tempo_as_bytes(120.0).to_vec()
                },
                MetaEvent {
                    tick: 0,
                    event_type: MetaEventType::KeySignature,
                    data: vec![0x00, 0x00] // c major
                },
                MetaEvent {
                    tick: 0,
                    event_type: MetaEventType::TimeSignature,
                    data: vec![0x04, 0x02, 0x18, 0x08] // 4:4
                }
            ];
        }

        {
            // ...and rebuild the tempo map
            let mut tempo_map = self.tempo_map.write().unwrap();
            tempo_map.meta_events = self.global_metas.clone();
            tempo_map.rebuild_tempo_map();
        }
    }

    pub fn validate_tracks(&mut self, track: u16) {
        let mut notes = self.notes.write().unwrap();
        let mut ch_evs = self.channel_events.write().unwrap();

        let last_len = notes.len();
        let new_len = track + 1;
        let len_change = new_len as i32 - last_len as i32;
        if len_change == 0 { return; }

        if len_change < 0 {
            for _ in 0..(-len_change) {
                let can_remove = notes.last().map_or(false, |n| n.is_empty())
                    && ch_evs.last().map_or(false, |c| c.is_empty());

                if can_remove {
                    notes.pop();
                    ch_evs.pop();
                } else {
                    break;
                }
            }
        } else {
            for _ in 0..len_change {
                notes.push(Vec::new());
                ch_evs.push(Vec::new());
            }

            assert!(notes.len() == ch_evs.len());
        }

        println!("Using {} tracks", notes.len());
    }
}