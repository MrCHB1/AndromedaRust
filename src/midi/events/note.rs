use crate::editor::util::MIDITick;

// channel and track is implied
#[derive(Default, Debug, Clone, Copy)]
pub struct Note {
    pub channel: u8,
    pub start: MIDITick,
    pub length: MIDITick,
    pub key: u8,
    pub velocity: u8
}

impl Note {
    #[inline(always)]
    pub fn start(&self) -> MIDITick {
        self.start
    }

    #[inline(always)]
    pub fn start_mut(&mut self) -> &mut MIDITick {
        &mut self.start
    }

    #[inline(always)]
    pub fn length(&self) -> MIDITick {
        self.length
    }

    #[inline(always)]
    pub fn length_mut(&mut self) -> &mut MIDITick {
        &mut self.length
    }

    #[inline(always)]
    pub fn key(&self) -> u8 {
        self.key
    }

    #[inline(always)]
    pub fn key_mut(&mut self) -> &mut u8 {
        &mut self.key
    }

    #[inline(always)]
    pub fn velocity(&self) -> u8 {
        self.velocity
    }

    #[inline(always)]
    pub fn velocity_mut(&mut self) -> &mut u8 {
        &mut self.velocity
    }

    #[inline(always)]
    pub fn channel(&self) -> u8 {
        self.channel
    }

    #[inline(always)]
    pub fn channel_mut(&mut self) -> &mut u8 {
        &mut self.channel
    }

    // extra stuff
    #[inline(always)]
    pub fn end(&self) -> MIDITick {
        self.start() + self.length()
    }

    #[inline(always)]
    pub fn set_start(&mut self, start: MIDITick) {
        *self.start_mut() = start;
    }

    #[inline(always)]
    pub fn set_length(&mut self, length: MIDITick) {
        *self.length_mut() = length;
    }

    #[inline(always)]
    pub fn set_end(&mut self, end: MIDITick) {
        self.set_length(end - self.start());
    }
}