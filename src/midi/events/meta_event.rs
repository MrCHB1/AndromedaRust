#![warn(unused)]
use crate::editor::util::MIDITick;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MetaEventType {
    SequenceNumber = 0x00,
    Text = 0x01,
    Copyright = 0x02,
    TrackName = 0x03,
    InstrumentName = 0x04,
    Lyric = 0x05,
    Marker = 0x06,
    CuePoint = 0x07,
    ProgramName = 0x08,
    DeviceName = 0x09,
    ChannelPrefix = 0x20,
    MIDIPort = 0x21,
    EndOfTrack = 0x2F,
    Tempo = 0x51,
    SMPTEOffset = 0x54,
    TimeSignature = 0x58,
    KeySignature = 0x59,
    SequencerSpecific = 0x7F
}

#[derive(Clone)]
pub struct MetaEvent {
    pub tick: MIDITick,
    pub event_type: MetaEventType,
    pub data: Vec<u8>
}

impl ToString for MetaEventType {
    fn to_string(&self) -> String {
        match self {
            MetaEventType::TimeSignature => "Time Signature",
            MetaEventType::Tempo => "Tempo",
            MetaEventType::KeySignature => "Key Signature",
            MetaEventType::Marker => "Marker",
            _ => ""
        }.to_string()
    }
}

impl MetaEvent {
    pub fn get_value_string(&self) -> String {
        match self.event_type {
            MetaEventType::Marker => {
                String::from_utf8_lossy(&self.data).to_string()
            },
            MetaEventType::Tempo => {
                let tempo = (self.data[2] as u32) | ((self.data[1] as u32) << 8) | ((self.data[0] as u32) << 16);
                let tempof = 60000000.0 / tempo as f32;
                tempof.to_string()
            },
            _ => { String::from("N/A") }
        }
    }
}