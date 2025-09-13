use crate::{editor::util::MIDITick, midi::{events::channel_event::{ChannelEvent, ChannelEventType}, midi_file::MIDIEvent}};

pub fn channel_to_midi_ev(channel_evs: &Vec<ChannelEvent>) -> Vec<MIDIEvent> {
    let mut last_time: MIDITick = 0;

    channel_evs.iter()
        .map(|ch_ev| {
            let curr_time = ch_ev.tick;
            let delta = curr_time - last_time;
            last_time = curr_time;

            let channel = ch_ev.channel;
            let data = match ch_ev.event_type {
                ChannelEventType::NoteOff(key) => vec![0x80 | channel, key, 0x00],
                ChannelEventType::NoteOn(key, vel) => vec![0x90 | channel, key, vel],
                ChannelEventType::NoteAftertouch(key, pressure) => vec![0xA0 | channel, key, pressure],
                ChannelEventType::Controller(ctrl, val) => vec![0xB0 | channel, ctrl, val],
                ChannelEventType::ProgramChange(program) => vec![0xC0 | channel, program],
                ChannelEventType::ChannelAftertouch(amount) => vec![0xD0 | channel, amount],
                ChannelEventType::PitchBend(lsb, msb) => vec![0xE0 | channel, lsb, msb],
            };

            MIDIEvent { delta, data }
        })
        .collect()
}

/// Merges two event sequences together, consuming both sequences.
pub fn merge_events(seq1: Vec<MIDIEvent>, seq2: Vec<MIDIEvent>) -> Vec<MIDIEvent> {
    let mut res = Vec::with_capacity(seq1.len() + seq2.len());
    let mut iter1 = seq1.into_iter().peekable();
    let mut iter2 = seq2.into_iter().peekable();

    while iter1.peek().is_some() || iter2.peek().is_some() {
        let next_ev = match (iter1.peek_mut(), iter2.peek_mut()) {
            (Some(ev1), Some(ev2)) => {
                if ev1.delta <= ev2.delta {
                    ev2.delta -= ev1.delta;
                    iter1.next().unwrap()
                } else {
                    ev1.delta -= ev2.delta;
                    iter2.next().unwrap()
                }
            }
            (Some(_), None) => {
                iter1.next().unwrap()
            },
            (None, Some(_)) => {
                iter2.next().unwrap()
            },
            (None, None) => break
        };
        res.push(next_ev);
    }

    res
}