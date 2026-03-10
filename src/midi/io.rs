pub mod buffered_reader;

pub enum MIDIParseStatus {
    ParseOK,
    ParseNotMIDI,
    ParseCorrupt,
    ParseError
}