pub enum ChannelEventType {
    NoteOff(u8),
    NoteOn(u8, u8),
    NoteAftertouch(u8, u8),
    Controller(u8, u8),
    ProgramChange(u8),
    ChannelAftertouch(u8),
    PitchBend(u8, u8)
}

pub struct ChannelEvent {
    pub tick: u64,
    pub channel: u8,
    pub event_type: ChannelEventType
}