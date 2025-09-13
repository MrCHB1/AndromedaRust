use std::{collections::{HashMap, HashSet, VecDeque}, fs::File, sync::{Arc, Mutex}};

use crate::{editor::util::MIDITick, midi::{events::{channel_event::{ChannelEvent, ChannelEventType}, meta_event::{MetaEvent, MetaEventType}, note::Note}, io::buffered_reader::BufferedByteReader}};

pub struct MIDITrackParser {
    pub reader: BufferedByteReader,
    // channels are separate
    pub note_events: Vec<Note>,
    pub channel_events: Vec<ChannelEvent>,
    pub meta_events: Vec<MetaEvent>,
    pub parse_success: bool,
    pub track_ended: bool,

    prev_cmd: u8,
    curr_tick: MIDITick,
    unended_notes: HashMap<usize, VecDeque<usize>>,
    curr_note_id: usize,
}

impl MIDITrackParser {
    pub fn new(stream: &Arc<Mutex<File>>, start: usize, length: usize) -> Self {
        /*let mut unended_notes: Vec<Vec<usize>> = Vec::with_capacity(256 << 4);
        for _ in 0..256<<4 { 
            unended_notes.push(Vec::new());
        }*/
        let unended_notes = HashMap::with_capacity(128 << 4);

        Self {
            reader: BufferedByteReader::new(stream, start, length, 100000).unwrap(),
            note_events: Vec::new(),
            channel_events: Vec::new(),
            meta_events: Vec::new(),
            prev_cmd: 0x00,
            curr_tick: 0,
            parse_success: true,
            track_ended: false,
            unended_notes,
            curr_note_id: 0,
        }
    }

    fn read_delta(&mut self) -> MIDITick {
        let mut n: MIDITick = 0;
        loop {
            let b = self.reader.read_byte().unwrap();
            n = (n << 7) | ((b & 0x7F) as MIDITick);
            if (b & 0x80) == 0x00 { break; }
        }
        n
    }

    pub fn parse_next(&mut self) {
        let delta = self.read_delta();
        self.curr_tick += delta;
        let mut command = self.reader.read_byte().unwrap();
        if command < 0x80 {
            self.reader.seek(-1, 1).unwrap();
            command = self.prev_cmd;
        }
        self.prev_cmd = command;
        let channel = command & 0x0F;
        match command & 0xF0 {
            0x80 => {
                // let key = self.reader.read_byte().unwrap();
                // let _ = self.reader.read_byte().unwrap();
                let (key, _) = self.reader.read_u8x2().unwrap();
                
                // set the end of the last note
                let un = self.unended_notes
                    .entry(((key as usize) << 4) | channel as usize)
                    .or_insert(VecDeque::new());

                if un.len() > 0 {
                    let n = un.pop_front().unwrap();
                    let note = &mut self.note_events[n];
                    note.set_length(self.curr_tick - note.start());
                }
            },
            0x90 => {
                // let key = self.reader.read_byte().unwrap();
                // let vel = self.reader.read_byte().unwrap();
                let (key, vel) = self.reader.read_u8x2().unwrap();

                let un = self.unended_notes
                    .entry(((key as usize) << 4) | channel as usize)
                    .or_insert(VecDeque::new());

                let note_evs_chn = &mut self.note_events;

                if vel > 0 {
                    // push a new note without a specified end
                    note_evs_chn.push(Note {
                        start: self.curr_tick,
                        length: MIDITick::MAX,
                        key,
                        velocity: vel,
                        channel: channel
                    });

                    //self.unended_notes[((key as usize) << 4) | channel as usize].push(self.curr_note_id[channel as usize]);
                    un.push_back(self.curr_note_id);
                    self.curr_note_id += 1;
                } else {
                    //let un = &mut self.unended_notes[((key as usize) << 4) | channel as usize];
                    if un.len() > 0 {
                        let n = un.pop_front().unwrap();
                        let note = &mut note_evs_chn[n];
                        note.set_length(self.curr_tick - note.start());
                    }
                }
            },
            // Note Aftertouch
            0xA0 => {
                // let key = self.reader.read_byte().unwrap();
                // let pressure = self.reader.read_byte().unwrap();
                let (key, pressure) = self.reader.read_u8x2().unwrap();

                self.channel_events.push(
                    ChannelEvent {
                        channel: channel,
                        tick: self.curr_tick, 
                        event_type: ChannelEventType::NoteAftertouch(key, pressure)
                    }
                );
            },
            // Controller
            0xB0 => {
                // let controller = self.reader.read_byte().unwrap();
                // let value = self.reader.read_byte().unwrap();
                let (controller, value) = self.reader.read_u8x2().unwrap();

                self.channel_events.push(
                    ChannelEvent {
                        channel: channel,
                        tick: self.curr_tick, 
                        event_type: ChannelEventType::Controller(controller, value)
                    }
                );
            },
            // Program change
            0xC0 => {
                let program = self.reader.read_byte().unwrap();
                self.channel_events.push(
                    ChannelEvent {
                        channel: channel,
                        tick: self.curr_tick, 
                        event_type: ChannelEventType::ProgramChange(program)
                    }
                );
            },
            // Channel Aftertouch
            0xD0 => {
                let amount = self.reader.read_byte().unwrap();
                self.channel_events.push(
                    ChannelEvent {
                        channel: channel,
                        tick: self.curr_tick, 
                        event_type: ChannelEventType::ChannelAftertouch(amount)
                    }
                );
            },
            // Pitch bend
            0xE0 => {
                // let lsb = self.reader.read_byte().unwrap();
                // let msb = self.reader.read_byte().unwrap();
                let (lsb, msb) = self.reader.read_u8x2().unwrap();

                self.channel_events.push(
                    ChannelEvent {
                        channel: channel,
                        tick: self.curr_tick,
                        event_type: ChannelEventType::PitchBend(lsb, msb)
                    }
                );
            },
            // Other/Meta events
            0xF0 => {
                match command {
                    0xFF => {
                        let meta_cmd = self.reader.read_byte().unwrap();
                        let meta_len = self.read_delta();
                        let mut meta_data: Vec<u8> = vec![0; meta_len as usize];
                        if meta_len > 0 { self.reader.read(&mut meta_data, meta_len as usize).unwrap(); }
                        match meta_cmd {
                            0x00 => {
                                if meta_len != 0x00 && meta_len != 0x02 { 
                                    self.parse_success = false;
                                    return;
                                }
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::SequenceNumber,
                                        data: meta_data
                                    }
                                );
                            },
                            0x01 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::Text,
                                        data: meta_data
                                    }
                                )
                            },
                            0x02 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::Copyright,
                                        data: meta_data
                                    }
                                )
                            },
                            0x03 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::TrackName,
                                        data: meta_data
                                    }
                                )
                            },
                            0x04 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::InstrumentName,
                                        data: meta_data
                                    }
                                )
                            },
                            0x05 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::Lyric,
                                        data: meta_data
                                    }
                                )
                            },
                            0x06 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::Marker,
                                        data: meta_data
                                    }
                                )
                            },
                            0x07 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::CuePoint,
                                        data: meta_data
                                    }
                                )
                            },
                            0x08 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::ProgramName,
                                        data: meta_data
                                    }
                                )
                            },
                            0x09 => {
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::DeviceName,
                                        data: meta_data
                                    }
                                )
                            },
                            0x20 => {
                                if meta_len != 0x01 {
                                    self.parse_success = false;
                                    return;
                                }
                                
                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::ChannelPrefix,
                                        data: meta_data
                                    }
                                )
                            },
                            0x21 => {
                                if meta_len != 0x01 {
                                    self.parse_success = false;
                                    return;
                                }

                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::MIDIPort,
                                        data: meta_data
                                    }
                                )
                            },
                            // end of track
                            0x2F => {
                                self.track_ended = true;
                                if meta_len != 0x00 {
                                    self.parse_success = false;
                                    return;
                                }
                                self.track_ended = true;
                            },
                            // tempo
                            0x51 => {
                                if meta_len != 0x03 {
                                    self.parse_success = false;
                                    return;
                                }

                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::Tempo,
                                        data: meta_data
                                    }
                                );
                            }
                            // SMPTEOffset
                            0x54 => {
                                if meta_len != 0x05 {
                                    self.parse_success = false;
                                    return;
                                }

                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::SMPTEOffset,
                                        data: meta_data
                                    }
                                );
                            }
                            // Time Signature
                            0x58 => {
                                if meta_len != 0x04 {
                                    self.parse_success = false;
                                    return;
                                }

                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::TimeSignature,
                                        data: meta_data
                                    }
                                );
                            },
                            // Key Signature
                            0x59 => {
                                if meta_len != 0x02 {
                                    self.parse_success = false;
                                    return;
                                }

                                self.meta_events.push(
                                    MetaEvent {
                                        tick: self.curr_tick,
                                        event_type: MetaEventType::KeySignature,
                                        data: meta_data
                                    }
                                );
                            },
                            0x7F => {

                            }
                            _ => {

                            }
                        }
                    }
                    0xF0 => {
                        let sysex_len = self.read_delta();
                        self.reader.skip_bytes(sysex_len as usize).unwrap();
                    }
                    0xF2 => {
                        self.reader.skip_bytes(2).unwrap();
                    }
                    0xF3 => {
                        self.reader.skip_bytes(1).unwrap();
                    },
                    0xF7 => {
                        let sysex_len = self.read_delta();
                        self.reader.skip_bytes(sysex_len as usize).unwrap();
                    },
                    _ => {}
                }
                
            },
            _ => {

            }
        }
    }
}