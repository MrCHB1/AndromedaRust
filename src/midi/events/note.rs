// channel and track is implied
#[derive(Default, Debug, Clone, Copy)]
pub struct Note {
    pub start: u32,
    pub length: u32,
    pub key: u8,
    pub velocity: u8
}