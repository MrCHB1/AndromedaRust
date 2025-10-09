#![warn(unused)]
pub trait MIDIAudioEngine {
    fn init_audio(&mut self);
    fn close_stream(&mut self);
    fn send_event(&mut self, raw_event: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
}