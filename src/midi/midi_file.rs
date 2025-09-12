use std::collections::VecDeque;
// 1. parse header first
use std::fs::File;
use std::io::{Read, Result, Seek, Write};
use std::sync::{Arc, Mutex};

use crate::editor::util::MIDITick;
use crate::midi::events::channel_event::ChannelEvent;
use crate::midi::events::meta_event::{MetaEvent, MetaEventType};
use crate::midi::events::note::Note;
use crate::midi::midi_track_parser::MIDITrackParser;

use itertools::Itertools;

pub struct MIDIFile {
    pub format: u16,
    pub trk_count: u16,
    pub ppq: u16,

    file_stream: Arc<Mutex<File>>,
    pub channel_events: Vec<Vec<ChannelEvent>>,
    pub meta_events: Vec<Vec<MetaEvent>>,
    // meta events that basically affect every track (like tempo, time signature, key signature...)
    // they go on the first track
    pub global_meta_events: Vec<MetaEvent>,
    pub notes: Vec<Vec<Vec<Note>>>,
}

pub struct MIDITrackPointer {
    pub start: u64,
    pub length: u32
}

impl MIDIFile {
    pub fn open(path: &str) -> Result<Self> {
        let file_stream = Arc::new(Mutex::new(File::open(path)?));
        let mut midi_file = Self {
            format: 0, trk_count: 0, ppq: 0, file_stream,
            channel_events: Vec::new(), meta_events: Vec::new(), global_meta_events: Vec::new(),
            notes: Vec::new()
        };

        // === parse header ===
        // check header first
        if midi_file.read_u32() != 0x4D546864 { panic!("Invalid header while reading MIDI File") }
        if midi_file.read_u32() != 0x00000006 { panic!("Expected header length to be 6.") }
        let format: u16 = midi_file.read_u16();
        let trk_count: u16 = midi_file.read_u16();
        let ppq: u16 = midi_file.read_u16();

        // === parse tracks in parallel
        let mut track_locations: Vec<MIDITrackPointer> = Vec::new();

        // first get all track locations
        {
            for _ in 0..trk_count {
                if midi_file.read_u32() != 0x4D54726B { panic!("Invalid track header!") }
                let length = midi_file.read_u32();

                let mut fs = midi_file.file_stream.lock().unwrap();
                let track_pos = (*fs).stream_position().unwrap();
                track_locations.push(MIDITrackPointer { start: track_pos, length });
                (*fs).seek(std::io::SeekFrom::Current(length as i64)).unwrap();
            }
        }

        // then make track parsers
        let mut track_parsers: Vec<MIDITrackParser> = Vec::new();
        for i in 0..trk_count as usize {
            let track_location = &track_locations[i];
            let fs = midi_file.file_stream.clone();
            track_parsers.push(MIDITrackParser::new(fs, track_location.start as usize, track_location.length as usize));
        }

        for i in 0..trk_count as usize {
            let track_parser = &mut track_parsers[i];
            while !track_parser.track_ended {
                track_parser.parse_next();
            }
            let channel_events = std::mem::take(&mut track_parser.channel_events);
            midi_file.channel_events.push(channel_events);
            let meta_events = std::mem::take(&mut track_parser.meta_events);
            midi_file.meta_events.push(meta_events);
            let notes = std::mem::take(&mut track_parser.note_events);
            midi_file.notes.push(notes);
        }

        midi_file.format = format;
        midi_file.trk_count = trk_count;
        midi_file.ppq = ppq;
        Ok(midi_file)
    }

    pub fn preprocess_meta_events(&mut self) {
        let unprocessed_metas = std::mem::take(&mut self.meta_events);
        // separate events to merge from non-mergable events (such as Track name, etc.)
        let mut non_mergeable: Vec<Vec<MetaEvent>> = Vec::new();
        let mut mergeable: Vec<Vec<MetaEvent>> = Vec::new();

        for track in unprocessed_metas.into_iter() {
            let mut nm_track: Vec<MetaEvent> = Vec::new();
            let mut m_track: Vec<MetaEvent> = Vec::new();
            
            for meta_ev in track.into_iter() {
                match meta_ev.event_type {
                    MetaEventType::Tempo | MetaEventType::TimeSignature | MetaEventType::KeySignature | MetaEventType::Lyric | MetaEventType::Marker => {
                        m_track.push(meta_ev);
                    },
                    _ => {
                        nm_track.push(meta_ev);
                    }
                }
            }
            non_mergeable.push(nm_track);
            mergeable.push(m_track);
        }

        self.global_meta_events = self.merge_meta_events(mergeable);
        println!("{}", self.global_meta_events.len());
        self.meta_events = non_mergeable;
    }

    fn merge_meta_seqs(&self, seq1: Vec<MetaEvent>, seq2: Vec<MetaEvent>) -> Vec<MetaEvent> {
        let mut enum1 = seq1.into_iter();
        let mut enum2 = seq2.into_iter();
        let mut e1 = enum1.next();
        let mut e2 = enum2.next();
        let mut res = Vec::new();

        loop {
            match e1 {
                Some(ref en1) => {
                    match e2 {
                        Some(ref en2) => {
                            if en1.tick < en2.tick {
                                res.push(e1.unwrap());
                                e1 = enum1.next();
                            } else {
                                res.push(e2.unwrap());
                                e2 = enum2.next();
                            }
                        }
                        None => {
                            res.push(e1.unwrap());
                            e1 = enum1.next();
                        }
                    }
                },
                None => {
                    if e2.is_none() { break; }
                    else {
                        res.push(e2.unwrap());
                        e2 = enum2.next();
                    }
                }
            }
        }

        res
    }

    fn merge_meta_events(&self, seq: Vec<Vec<MetaEvent>>) -> Vec<MetaEvent> {
        let mut b1 = seq.into_iter().collect::<Vec<_>>();
        let mut b2 = Vec::new();
        if b1.len() == 0 {
            return Vec::new();
        }
        while b1.len() > 1 {
            while b1.len() > 0 {
                if b1.len() == 1 {
                    b2.push(b1.remove(0));
                } else {
                    b2.push(self.merge_meta_seqs(b1.remove(0), b1.remove(0)));
                }
            }
            b1 = b2;
            b2 = Vec::new();
        }
        b1.remove(0)
    }

    fn read_u16(&mut self) -> u16 {
        let mut b = [0u8; 2];
        (self.file_stream.lock().unwrap()).read(&mut b).unwrap();
        return ((b[0] as u16) << 8) | (b[1] as u16);
    }

    fn read_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        (self.file_stream.lock().unwrap()).read(&mut b).unwrap();
        return ((b[0] as u32) << 24) |
                ((b[1] as u32) << 16) |
                ((b[2] as u32) << 8) |
                ((b[3] as u32) << 0);
    }
}

pub struct MIDIEvent {
    pub delta: MIDITick, // not VLQ
    pub data: Vec<u8>
}

impl MIDIEvent {
    pub fn get_vlq(&self) -> Vec<u8> {
        let mut delta = self.delta;
        let mut res = Vec::new();
        res.push((delta & 0x7F) as u8);
        delta >>= 7;

        while delta > 0 {
            res.push(((delta & 0x7F) as u8) | 0x80);
            delta >>= 7;
        }
        
        res.reverse();
        return res;
    }
}

pub struct MIDIFileWriter {
    ppq: u16,
    track_count: u16,
    tracks: Vec<Vec<MIDIEvent>>
}

impl MIDIFileWriter {
    pub fn new(ppq: u16) -> Self {
        Self {
            ppq,
            track_count: 0,
            tracks: Vec::new()
        }
    }

    pub fn new_track(&mut self) {
        self.tracks.push(Vec::new());
        self.track_count += 1;
    }

    pub fn flush_evs_to_track(&mut self, events: Vec<MIDIEvent>) {
        self.tracks[self.track_count as usize - 1].extend( events);
    }

    pub fn end_track(&mut self) {
        self.tracks[self.track_count as usize - 1].push(MIDIEvent {
            delta: 0,
            data: vec![0xFF, 0x2F, 0x00]
        });
    }
    
    pub fn flush_global_metas(&mut self, meta_events: &Vec<MetaEvent>) {
        self.new_track();
        let mut seq: Vec<MIDIEvent> = Vec::new();
        let mut prev_time = 0;
        for meta_event in meta_events.iter() {
            let meta_ev_code = &meta_event.event_type;
            seq.push(MIDIEvent {
                delta: meta_event.tick - prev_time,
                data: [vec![
                    0xFF, *meta_ev_code as u8, meta_event.data.len() as u8, 
                ], meta_event.data.clone()].concat()
            });
            prev_time = meta_event.tick;
        }
        println!("{}", seq.len());
        self.flush_evs_to_track(seq);
        self.end_track();
    }

    // assuming notes is 16 vectors
    pub fn add_notes_to_midi(&mut self, notes: &Vec<Vec<Note>>) {
        let mut curr_chan = 0;
        for channel_note in notes.iter() {
            if channel_note.len() == 0 { curr_chan += 1; continue; } // we don't want empty tracks

            self.new_track();
            let conv = self.notes_to_events(channel_note.iter().sorted_by_key(|n| n.start).collect::<Vec<_>>(), curr_chan);
            self.flush_evs_to_track(conv);
            self.end_track();
            curr_chan += 1;
        }
    }

    fn notes_to_events(&self, notes: Vec<&Note>, channel: u8) -> Vec<MIDIEvent> {
        let mut seq: Vec<MIDIEvent> = Vec::new();
        let mut note_off_times: VecDeque<(MIDIEvent, MIDITick)> = VecDeque::new();
        let mut prev_time = 0;

        for note in notes.iter() {
            while !note_off_times.is_empty() && note_off_times.front().unwrap().1 <= note.start {
                let mut ev = note_off_times.pop_front().unwrap();
                ev.0.delta = ev.1 - prev_time;
                seq.push(ev.0);
                prev_time = ev.1;
            }

            seq.push(MIDIEvent {
                delta: note.start - prev_time,
                data: vec![0x90 | channel, note.key, note.velocity]
            });
            prev_time = note.start;
            let time = note.start + note.length;
            let off = (MIDIEvent {
                delta: 0,
                data: vec![0x80 | channel, note.key, 0x00]
            }, time);
            let mut pos = note_off_times.len() / 2;
            if note_off_times.is_empty() { note_off_times.push_back(off); }
            else {
                let mut jump = note_off_times.len() / 4;
                loop {
                    if jump <= 0 { jump = 1; }
                    // if pos < 0 { pos = 0; }
                    if pos >= note_off_times.len() { pos = note_off_times.len() - 1; }
                    let u = &note_off_times[pos];
                    if u.1 >= time {
                        if pos == 0 || note_off_times[pos - 1].1 < time {
                            note_off_times.insert(pos, (MIDIEvent {
                                delta: 0,
                                data: vec![0x80 | channel, note.key, 0x00]
                            }, time));
                            break;
                        } else { pos -= jump; }
                    } else {
                        if pos == note_off_times.len() - 1 {
                            note_off_times.push_back((MIDIEvent {
                                delta: 0,
                                data: vec![0x80 | channel, note.key, 0x00]
                            }, time));
                            break;
                        } else { pos += jump; }
                    }
                    jump /= 2;
                }
            }
        }

        for (mut ev, time) in note_off_times.into_iter() {
            ev.delta = time - prev_time;
            seq.push(ev);
            prev_time = time;
        }

        return seq;
    }

    pub fn write_midi(&self, path: &str) -> Result<()> {
        let mut file = File::create(path)?;
        
        // header
        self.write_u32(&mut file, 0x4D546864)?;
        // header length
        self.write_u32(&mut file, 6)?;
        self.write_u16(&mut file, 1)?; // format
        self.write_u16(&mut file, self.track_count)?;
        self.write_u16(&mut file, self.ppq)?;

        // iterate through tracks
        for track in self.tracks.iter() {
            // track header
            self.write_u32(&mut file, 0x4D54726B)?;
            // count bytes
            /*let num_bytes: usize = track.iter().map(|n| n.data.len()).sum();
            self.write_u32(&mut file, num_bytes as u32)?;
            for ev in track {
                let delta = ev.get_vlq();
                file.write(&delta)?;
                file.write(&ev.data)?;
            }*/
            let mut buf: Vec<u8> = Vec::new();
            for ev in track {
                let delta = ev.get_vlq();
                buf.extend(delta);
                buf.extend(&ev.data);
            }
            self.write_u32(&mut file, buf.len() as u32)?;
            file.write(&buf)?;
        }

        Ok(())
    }

    fn write_u32(&self, writer: &mut File, val: u32) -> Result<()> {
        writer.write(&[
            ((val & 0xFF000000) >> 24) as u8,
            ((val & 0xFF0000) >> 16) as u8,
            ((val & 0xFF00) >> 8) as u8,
            (val & 0xFF) as u8
        ])?;
        Ok(())
    }

    fn write_u16(&self, writer: &mut File, val: u16) -> Result<()> {
        writer.write(&[
            ((val & 0xFF00) >> 8) as u8,
            (val & 0xFF) as u8
        ])?;
        Ok(())
    }
}