#[derive(Copy, Clone, PartialEq)]
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

pub struct MetaEvent {
    pub tick: u64,
    pub event_type: MetaEventType,
    pub data: Vec<u8>
}