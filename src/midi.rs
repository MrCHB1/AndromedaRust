pub mod midi_file;
pub mod events;
pub mod midi_track_parser;
pub mod io;
pub mod midi_track;

pub const MIDI_KEY_MIN: u8 = 0;
pub const MIDI_KEY_MIN_SIGNED: i8 = 0;
pub const MIDI_KEY_MAX: u8 = 127;
pub const MIDI_KEY_MAX_SIGNED: i8 = 127;