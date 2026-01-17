use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::fs::File;
use std::io::{BufWriter, Read, Result, Seek, Write};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::editor::util::MIDITick;
use crate::midi::events::channel_event::ChannelEvent;
use crate::midi::events::mergers::{channel_to_midi_ev, merge_events};
use crate::midi::events::meta_event::{MetaEvent, MetaEventType};
use crate::midi::events::note::Note;
use crate::midi::midi_track::MIDITrack;
use crate::midi::midi_track_parser::MIDITrackParser;

use itertools::Itertools;
use rayon::prelude::*;

// dude i dont think i can optimize this even further LOL

pub struct MIDIFile {
    pub format: u16,
    pub trk_count: u16,
    pub ppq: u16,

    // pub channel_events: Vec<Vec<ChannelEvent>>,
    // pub meta_events: Vec<Vec<MetaEvent>>,
    // meta events that basically affect every track (like tempo, time signature, key signature...)
    // they go on the first track
    pub global_meta_events: Vec<MetaEvent>,
    // pub notes: Vec<Vec<Note>>,

    pub tracks: Vec<MIDITrack>,

    // some useful settings
    track_discarding: bool,

    // counters
    per_track_metas: usize,
    global_metas: usize
}

pub struct MIDITrackPointer {
    pub start: u64,
    pub length: u32
}

impl MIDIFile {
    pub fn new() -> Self {
        Self {
            format: 0,
            trk_count: 0,
            ppq: 0,
            // file_stream: Default::default(),
            // channel_events: Vec::new(),
            // meta_events: Vec::new(),
            global_meta_events: Vec::new(),
            // notes: Vec::new(),
            tracks: Vec::new(),
            track_discarding: false,

            // info
            per_track_metas: 0,
            global_metas: 0
        }
    }

    pub fn with_track_discarding<'a>(&'a mut self, value: bool) -> &'a mut Self {
        self.track_discarding = value;
        self
    }
    
    pub fn open<'a>(&'a mut self, path: &str) -> Result<&'a mut Self> {
        let file_stream = Arc::new(Mutex::new(File::open(path)?));

        // === parse header ===
        let (format, trk_count, ppq) = {
            let mut fs = file_stream.lock().unwrap();

            let (header, length) = self.read_u32x2(&mut fs);
            assert!(header == 0x4D546864 && length == 6, "Invalid file header.");

            self.read_u16x3(&mut fs)
        };

        // === parse tracks in parallel
        let mut track_locations: Vec<MIDITrackPointer> = Vec::with_capacity(trk_count as usize);
        self.tracks = Vec::with_capacity(trk_count as usize);

        // first get all track locations
        {
            let mut fs = file_stream.lock().unwrap();
            for _ in 0..trk_count {
                let (header, length) = self.read_u32x2(&mut fs);
                assert!(header == 0x4D54726B, "Invalid track header!");

                let track_pos = fs.stream_position().unwrap();
                track_locations.push(MIDITrackPointer { start: track_pos, length });
                fs.seek(std::io::SeekFrom::Current(length as i64)).unwrap();
            }
        }

        // populate parsers
        let mut track_parsers: Vec<MIDITrackParser> = Vec::with_capacity(trk_count as usize);

        for i in 0..trk_count as usize {
            let track_location = &track_locations[i];
            track_parsers.push(MIDITrackParser::new(&file_stream, track_location.start as usize, track_location.length as usize));
            
            self.tracks.push(MIDITrack::new_empty());
            // self.channel_events.push(Vec::new());
            // self.meta_events.push(Vec::new());
            // self.notes.push(Vec::new());
        }
        
        // let channel_events = &mut self.channel_events;
        // let meta_events = &mut self.meta_events;
        // let notes = &mut self.notes;

        // instead of putting it all into one parallel iterator, i've split it in two
        // one for if track discarding is on, and one for no track discarding. 
        // this is to avoid any uneccessary extra allocations
        track_parsers.par_iter_mut()
            .zip(self.tracks.par_iter_mut())
            .for_each(|(parser, track)| {
                let MIDITrack { muted: _, channel_events, meta_events, notes } = track;
                Self::parse_track(parser, notes, channel_events, meta_events);
            });
        /*if self.track_discarding {
            let mut tracks_to_discard: Vec<bool> = vec![false; trk_count as usize];
            let mut idx = 0;

            tracks_to_discard.par_iter_mut()
                .zip(track_parsers.par_iter_mut()
                .zip(notes.par_iter_mut()
                .zip(channel_events.par_iter_mut()
                .zip(meta_events.par_iter_mut()))))
                .enumerate()
                .for_each(|(track, (discard, (parser, (notes, (channel_evs, meta_evs)))))| {
                    Self::parse_track(parser, notes, channel_evs, meta_evs);

                    *discard = parser.note_events.is_empty() && track > 0;
                });

            notes.retain(|_| { let keep = !tracks_to_discard[idx]; idx += 1; keep }); idx = 0;
            channel_events.retain(|_| { let keep = !tracks_to_discard[idx]; idx += 1; keep }); idx = 0;
            meta_events.retain(|_| { let keep = !tracks_to_discard[idx]; idx += 1; keep });

            println!("Removed {} tracks.", tracks_to_discard.iter().filter(|&&d| d).count());
        } else {
            track_parsers.par_iter_mut()
                .zip(notes.par_iter_mut()
                .zip(channel_events.par_iter_mut()
                .zip(meta_events.par_iter_mut())))
                .for_each(|(parser, (notes, (channel_evs, meta_evs)))| {
                    Self::parse_track(parser, notes, channel_evs, meta_evs);
                });
        }*/

        self.format = format;
        self.trk_count = trk_count;
        self.ppq = ppq;

        Ok(self)
    }

    #[inline(always)]
    fn parse_track(parser: &mut MIDITrackParser, notes: &mut Vec<Note>, channel_evs: &mut Vec<ChannelEvent>, meta_evs: &mut Vec<MetaEvent>) {
        while !parser.track_ended {
            parser.parse_next();
        }
        
        *notes = std::mem::take(&mut parser.note_events);
        *channel_evs = std::mem::take(&mut parser.channel_events);
        *meta_evs = std::mem::take(&mut parser.meta_events);
    }

    pub fn preprocess_meta_events(&mut self) {

        // separate events to merge from non-mergable events (such as Track name, etc.)
        let mut mergeable: Vec<Vec<MetaEvent>> = Vec::with_capacity(self.tracks.len());

        for track in self.tracks.iter_mut() {
            let mut m_track: Vec<MetaEvent> = Vec::new();
            
            let meta_evs_ = track.get_meta_events_mut();
            let meta_evs = std::mem::take(meta_evs_);
            
            for meta_ev in meta_evs.into_iter() {
                match meta_ev.event_type {
                    MetaEventType::Tempo | 
                    MetaEventType::TimeSignature | 
                    MetaEventType::KeySignature | 
                    MetaEventType::Lyric | 
                    MetaEventType::Marker => {
                        m_track.push(meta_ev);
                    },
                    _ => {
                        meta_evs_.push(meta_ev);
                    }
                }
            }

            mergeable.push(m_track);
        }

        self.global_meta_events = self.merge_meta_events(mergeable);
    }

    fn merge_meta_seqs(&self, seq1: Vec<MetaEvent>, seq2: Vec<MetaEvent>) -> Vec<MetaEvent> {
        let mut res = Vec::with_capacity(seq1.len() + seq2.len());

        let mut iter1 = seq1.into_iter().peekable();
        let mut iter2 = seq2.into_iter().peekable();

        while iter1.peek().is_some() || iter2.peek().is_some() {
            let next_ev = match (iter1.peek(), iter2.peek()) {
                (Some(ev1), Some(ev2)) => {
                    if ev1.tick <= ev2.tick { iter1.next().unwrap() } else { iter2.next().unwrap() }
                },
                (Some(_), None) => iter1.next().unwrap(),
                (None, Some(_)) => iter2.next().unwrap(),
                (None, None) => break
            };
            res.push(next_ev);
        }

        res
    }

    fn merge_meta_events(&self, seq: Vec<Vec<MetaEvent>>) -> Vec<MetaEvent> {
        if seq.is_empty() {
            return Vec::new();
        }

        let mut queue: VecDeque<Vec<MetaEvent>> = seq.into_iter().collect();

        while queue.len() > 1 {
            let seq1 = queue.pop_front().unwrap();
            let seq2 = queue.pop_front().unwrap();
            queue.push_back(self.merge_meta_seqs(seq1, seq2));
        }

        queue.pop_front().unwrap_or_default()
    }

    /*fn read_u16(&mut self) -> u16 {
        let mut b = [0u8; 2];
        (self.file_stream.lock().unwrap()).read(&mut b).unwrap();
        Self::bytes_to_u16(&b)
    }

    fn read_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        (self.file_stream.lock().unwrap()).read(&mut b).unwrap();
        Self::bytes_to_u32(&b)
    }*/

    // only ever used for the file header lmao
    fn read_u16x3(&mut self, stream: &mut MutexGuard<'_ , File>) -> (u16, u16, u16) {
        let mut b = [0u8; 6];
        stream.read(&mut b).unwrap();
        let (a, bc) = b.split_at(2);
        let (b, c) = bc.split_at(2);
        (Self::bytes_to_u16(a), 
         Self::bytes_to_u16(b),
         Self::bytes_to_u16(c))
    }

    fn read_u32x2(&mut self, stream: &mut MutexGuard<'_ , File>) -> (u32, u32) {
        let mut b = [0u8; 8];
        stream.read(&mut b).unwrap();
        let (a, b) = b.split_at(4);
        (Self::bytes_to_u32(a),
         Self::bytes_to_u32(b))
    }

    fn bytes_to_u16(bytes: &[u8]) -> u16 {
        ((bytes[0] as u16) << 8) | (bytes[1] as u16)
    }

    fn bytes_to_u32(bytes: &[u8]) -> u32 {
        ((bytes[0] as u32) << 24) |
        ((bytes[1] as u32) << 16) |
        ((bytes[2] as u32) << 8) |
        ((bytes[3] as u32) << 0)
    }
}

pub struct MIDIEvent {
    pub delta: MIDITick, // not VLQ
    pub data: Vec<u8>
}

impl MIDIEvent {
    /*pub fn get_vlq(&self) -> Vec<u8> {
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
    }*/

    pub fn write_to<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        self.write_delta_to(w)?;
        w.write_all(&self.data)?;
        Ok(())
    }

    pub fn write_delta_to<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        let mut delta = self.delta;

        let mut buf: [u8; 5] = [0; 5];
        let mut i: usize = 0;
        buf[0] = (delta & 0x7F) as u8;
        delta >>= 7;

        while delta > 0 {
            i += 1;

            buf[i] = ((delta & 0x7F) as u8) | 0x80;
            delta >>= 7;
        }

        for idx in (0..=i).rev() {
            w.write_all(&[buf[idx]])?;
        }
    
        Ok(())
    }

    pub fn vlq_len(&self) -> usize {
        let mut delta = self.delta;
        if delta == 0 { return 1; }

        let mut len = 0;
        while delta > 0 {
            len += 1;
            delta >>= 7;
        }

        len
    }
}

struct TimedEvent {
    time: MIDITick,
    event: MIDIEvent 
}

impl Eq for TimedEvent {}

impl PartialEq for TimedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Ord for TimedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.time.cmp(&self.time)
    }
}

impl PartialOrd for TimedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
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

    /// Adds a new empty track and returns its index.
    pub fn new_track(&mut self) -> usize {
        self.tracks.push(Vec::new());
        self.track_count += 1;
        self.track_count as usize - 1
    }

    /// Appends a ready-made track into this writer and returns the index.
    pub fn append_track(&mut self, track: Vec<MIDIEvent>) -> usize {
        self.tracks.push(track);
        self.track_count += 1;
        self.track_count as usize - 1
    }

    pub fn into_single_track(self) -> Vec<MIDIEvent> {
        assert!(self.tracks.len() == 1, "Writer must contain exactly 1 track. self.tracks.len() = {}", self.tracks.len());
        self.tracks.into_iter().next().unwrap()
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

    pub fn add_notes_to_midi(&mut self, notes: &Vec<Note>) {
        if notes.is_empty() { return; }

        // self.new_track();
        let conv = self.notes_to_events(notes.iter().sorted_by_key(|n| n.start).collect::<Vec<_>>());
        self.flush_evs_to_track(conv);
        // self.end_track();
    }

    pub fn add_notes_with_other_events(&mut self, notes: &Vec<Note>, events: &Vec<ChannelEvent>) {
        if notes.is_empty() { return; }

        if events.is_empty() {
            self.add_notes_to_midi(notes);
            return;
        }

        // self.new_track();
        let notes_conv = self.notes_to_events(notes.iter().sorted_by_key(|n| n.start).collect::<Vec<_>>());
        let chans_cov = channel_to_midi_ev(events);
        let merged = merge_events(notes_conv, chans_cov);
        self.flush_evs_to_track(merged);
        // self.end_track();
    }

    fn notes_to_events(&self, notes: Vec<&Note>) -> Vec<MIDIEvent> {
        let mut seq: Vec<MIDIEvent> = Vec::new();
        let mut note_offs: BinaryHeap<TimedEvent> = BinaryHeap::new();
        let mut prev_time = 0;

        for note in notes {
            while let Some(top) = note_offs.peek() {
                if top.time > note.start { break; }

                let TimedEvent { time, mut event } = note_offs.pop().unwrap();
                event.delta = time - prev_time;
                prev_time = time;
                seq.push(event);
            }

            seq.push(MIDIEvent {
                delta: note.start() - prev_time,
                data: vec![
                    0x90 | note.channel(),
                    note.key(),
                    note.velocity()
                ]
            });

            prev_time = note.start();

            note_offs.push(TimedEvent {
                time: note.start() + note.length(),
                event: MIDIEvent {
                    delta: 0,
                    data: vec![
                        0x80 | note.channel(),
                        note.key,
                        0x00
                    ]
                }
            });
        }

        while let Some(TimedEvent { time, mut event }) = note_offs.pop() {
            event.delta = time - prev_time;
            prev_time = time;
            seq.push(event);
        }

        return seq;
    }

    pub fn write_midi(&self, path: &str) -> Result<()> {
        let file = File::create(path)?;
        let mut out = BufWriter::with_capacity(16 * 1024 * 1024, file);
        
        // header
        self.write_u32(&mut out, 0x4D546864)?;

        // header length
        self.write_u32(&mut out, 6)?;
        self.write_u16(&mut out, 1)?; // format
        self.write_u16(&mut out, self.track_count)?;
        self.write_u16(&mut out, self.ppq)?;

        // iterate through tracks
        for track in self.tracks.iter() {
            let mut track_len: u32 = 0;

            for ev in track.iter() {
                track_len = track_len.checked_add(
                    (ev.vlq_len() + ev.data.len()) as u32
                ).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "track length overflow"))?;
            }

            self.write_u32(&mut out, 0x4D54726B)?;
            self.write_u32(&mut out, track_len)?;

            for ev in track.iter() {
                ev.write_to(&mut out)?;
            }
        }

        out.flush()?;
        Ok(())
    }

    fn write_u32<W: Write>(&self, writer: &mut W, val: u32) -> Result<()> {
        writer.write(&[
            ((val & 0xFF000000) >> 24) as u8,
            ((val & 0xFF0000) >> 16) as u8,
            ((val & 0xFF00) >> 8) as u8,
            (val & 0xFF) as u8
        ])?;
        Ok(())
    }

    fn write_u16<W: Write>(&self, writer: &mut W, val: u16) -> Result<()> {
        writer.write(&[
            ((val & 0xFF00) >> 8) as u8,
            (val & 0xFF) as u8
        ])?;
        Ok(())
    }
}