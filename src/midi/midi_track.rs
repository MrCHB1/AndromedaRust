use crate::midi::events::{channel_event::ChannelEvent, meta_event::MetaEvent, note::Note};

#[derive(Clone, Default)]
pub struct MIDITrack {
    pub muted: bool,
    pub channel_events: Vec<ChannelEvent>,
    pub meta_events: Vec<MetaEvent>,
    pub notes: Vec<Note>
}

impl MIDITrack {
    pub fn new(notes: Vec<Note>, channel_events: Vec<ChannelEvent>, meta_events: Vec<MetaEvent>) -> Self {
        Self {
            notes,
            channel_events,
            meta_events,
            muted: false
        }
    }

    pub fn new_empty() -> Self {
        Self {
            notes: Vec::new(),
            channel_events: Vec::new(),
            meta_events: Vec::new(),
            muted: false
        }
    }

    #[inline(always)]
    pub fn get_notes(&self) -> &Vec<Note> {
        &self.notes
    }

    #[inline(always)]
    pub fn get_notes_mut(&mut self) -> &mut Vec<Note> {
        &mut self.notes
    }

    #[inline(always)]
    pub fn get_channel_evs(&self) -> &Vec<ChannelEvent> {
        &self.channel_events
    }

    #[inline(always)]
    pub fn get_channel_evs_mut(&mut self) -> &mut Vec<ChannelEvent> {
        &mut self.channel_events
    }

    #[inline(always)]
    pub fn get_meta_events(&self) -> &Vec<MetaEvent> {
        &self.meta_events
    }

    #[inline(always)]
    pub fn get_meta_events_mut(&mut self) -> &mut Vec<MetaEvent> {
        &mut self.meta_events
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty() && self.channel_events.is_empty()
    }

    pub fn clear_track(&mut self) {
        self.notes.clear();
        self.channel_events.clear();
        self.meta_events.clear();
    }
}