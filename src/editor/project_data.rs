// houses all information such as notes, tempos, control/meta events, etc.
// kinda like midi_file.rs but editable lol

use crate::midi::{events::{channel_event::ChannelEvent, meta_event::{MetaEvent, MetaEventType}, note::Note}, midi_file};
use std::sync::{Arc, Mutex};

pub fn tempo_as_bytes(tempo: f32) -> [u8; 3] {
    let tempo_conv = (60000000.0 / tempo) as u32;
    return [
        ((tempo_conv >> 16) & 0xFF) as u8,
        ((tempo_conv >> 8) & 0xFF) as u8,
        (tempo_conv & 0xFF) as u8
    ];
}

pub fn bytes_as_tempo(bytes: &[u8]) -> f32 {
    let bytes_conv = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
    return 60000000.0 / (bytes_conv as f32);
}

pub struct ProjectInfo {
    pub name: &'static str,
    pub author: &'static str,
    pub description: &'static str,
    pub ppq: u16
}

impl Default for ProjectInfo {
    fn default() -> Self {
        Self {
            name: "",
            author: "",
            description: "",
            ppq: 960
        }
    }
}

#[derive(Default)]
pub struct ProjectData {
    pub project_info: ProjectInfo,
    // 16 channels per track. each channel contains vector of notes
    pub notes: Arc<Mutex<Vec<Vec<Vec<Note>>>>>,
    pub global_metas: Arc<Mutex<Vec<MetaEvent>>>,
    pub channel_events: Arc<Mutex<Vec<Vec<ChannelEvent>>>>
}

impl ProjectData {
    /*pub fn new() -> Self {
        let start_meta = vec![
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
                data: vec![0x04, 0x02, 0x18, 0x08]
            }
        ];

        Self {
            project_info: Default::default(),
            notes: Arc::new(Mutex::new(Vec::new())),
            global_metas: Arc::new(Mutex::new(start_meta))
        }
    }*/

    pub fn import_from_midi_file(&mut self, path: String) {
        let project_info = &mut self.project_info;
        let mut file = midi_file::MIDIFile::open(&path).unwrap();
        //self.notes = Arc::new(Mutex::new(std::mem::take(&mut file.notes)));
        {
            let mut notes = self.notes.lock().unwrap();
            let mut global_metas = self.global_metas.lock().unwrap();
            let mut channel_events = self.channel_events.lock().unwrap();
            file.preprocess_meta_events(); // will merge specific meta events into one track

            *notes = std::mem::take(&mut file.notes);
            *global_metas = std::mem::take(&mut file.global_meta_events);
            *channel_events = std::mem::take(&mut file.channel_events);
        }
        
        project_info.name = "";
        project_info.author = "";
        project_info.description = "";
        project_info.ppq = file.ppq;
    }

    pub fn new_empty_project(&mut self) {
        let project_info = &mut self.project_info;
        project_info.name = "";
        project_info.author = "";
        project_info.description = "";
        project_info.ppq = 960;
        
        {
            let mut notes = self.notes.lock().unwrap();
            notes.clear();
            // initialize an empty track
            let mut track: Vec<Vec<Note>> = Vec::new();
            for _ in 0..16 {
                track.push(Vec::new());
            }
            notes.push(track);
        }

        // initialize default meta events
        {
            let mut global_metas = self.global_metas.lock().unwrap();
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
        /*self.global_metas = Arc::new(Mutex::new(vec![
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
        ]));*/
    }
}