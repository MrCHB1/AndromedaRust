use std::{collections::{BinaryHeap, LinkedList, VecDeque}, sync::{atomic::{AtomicBool, AtomicU32, Ordering}, Arc, Mutex, RwLock}, thread::JoinHandle, time::{Duration, Instant}};

use crate::{audio::{midi_audio_engine::MIDIAudioEngine, midi_devices::MIDIDevices}, editor::{project_data::bytes_as_tempo, util::{bin_search_notes_exact, AtomicMIDITick, MIDITick, MIDITickAtomic}}, midi::events::{channel_event::{ChannelEvent, ChannelEventType}, meta_event::{MetaEvent, MetaEventType}, note::Note}};
use crossbeam::channel::{bounded, Receiver, RecvTimeoutError, Sender};
use std::thread;
use ordered_float::NotNan;
use std::cmp::Reverse;
use std::sync::MutexGuard;

#[derive(Eq, PartialEq)]
enum MidiEvent {
    NoteOn { channel: u8, key: u8, velocity: u8},
    NoteOff { channel: u8, key: u8, velocity: u8 },
    Control { channel: u8, controller: u8, value: u8 },
    PitchBend { channel: u8, lsb: u8, msb: u8 }
}

#[derive(Eq)]
struct Scheduled {
    time: MIDITick,
    event: MidiEvent,
}

impl PartialEq for Scheduled {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}
impl PartialOrd for Scheduled {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (&self.time).partial_cmp(&other.time)
    }
}
impl Ord for Scheduled {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.time).partial_cmp(&other.time).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Scheduled {
    #[inline(always)]
    pub fn get_time(&self) -> &MIDITick {
        &self.time
    }
}

struct ScheduledSequence {
    sequence: Vec<Scheduled>,
    min_index: usize
}

impl ScheduledSequence {
    pub fn new(scheduled_at_least: usize) -> Self {
        Self {
            sequence: Vec::with_capacity(scheduled_at_least),
            min_index: 0
        }
    }

    pub fn insert(&mut self, scheduled_event: Scheduled) {
        self.sequence.push(scheduled_event);
        let idx = self.last();

        if self.get(idx).unwrap().get_time() <= self.peek().unwrap().get_time() {
            self.min_index = idx;
        }
    }

    #[inline(always)]
    fn get(&self, index: usize) -> Option<&Scheduled> {
        if !self.sequence.is_empty() && index <= self.last() {
            Some(&self.sequence[index])
        } else {
            None
        }
    }

    #[inline(always)]
    fn last(&self) -> usize {
        if self.sequence.is_empty() { 0 }
        else { self.sequence.len() - 1 }
    }

    #[inline(always)]
    pub fn peek(&self) -> Option<&Scheduled> {
        self.get(self.min_index)
    }

    pub fn pop(&mut self) -> Option<Scheduled> {
        let earliest_scheduled = self.sequence.swap_remove(self.min_index);

        // unfortunately will be O(n), but thats ok because it's not even that expensive
        let mut new_min_index = 0;
        for (idx, scheduled) in self.sequence.iter().enumerate() {
            if scheduled.get_time() <= self.get(new_min_index).unwrap().get_time() {
                new_min_index = idx;
            }
        }

        self.min_index = new_min_index;

        Some(earliest_scheduled)
    }
}

#[derive(Clone, Copy)]
enum MidiEventBatchSize {
    Unlimited,
    BatchSize(usize)
}

impl Default for MidiEventBatchSize {
    fn default() -> Self {
        MidiEventBatchSize::BatchSize(4096)
    }
}

pub struct PlaybackManager {
    pub notes: Arc<RwLock<Vec<Vec<Note>>>>,
    pub meta_events: Arc<Mutex<Vec<MetaEvent>>>,
    pub channel_events: Arc<Mutex<Vec<Vec<ChannelEvent>>>>,
    pub device: Arc<Mutex<dyn MIDIAudioEngine + Send>>,
    pub ppq: u16,

    tx: Sender<MidiEvent>,
    rx: Receiver<MidiEvent>,
    notify_tx: Arc<Sender<()>>,
    notify_rx: Arc<Receiver<()>>,
    stop_playback: Arc<AtomicBool>,
    pub playing: bool,
    pub playback_start_pos: MIDITick,
    pub playback_pos_ticks: Arc<MIDITickAtomic>,

    start_time: Arc<Mutex<Instant>>,
    start_pos_secs_from_ticks: f32,
    tempo_map: Arc<RwLock<Vec<(MIDITick, f32)>>>,
    batch_size: Arc<Mutex<MidiEventBatchSize>>,
}

impl PlaybackManager {
    pub fn new(
        device: Arc<Mutex<dyn MIDIAudioEngine + Send>>,
        notes: Arc<RwLock<Vec<Vec<Note>>>>,
        meta_events: Arc<Mutex<Vec<MetaEvent>>>,
        channel_events: Arc<Mutex<Vec<Vec<ChannelEvent>>>>,
    ) -> Self {
        let (tx, rx) = bounded(100000);
        let (notify_tx, notify_rx) = bounded::<()>(1);
        
        Self {
            device, notes, meta_events, channel_events,
            tx, rx,
            notify_tx: Arc::new(notify_tx),
            notify_rx: Arc::new(notify_rx),
            stop_playback: Arc::new(AtomicBool::new(false)),
            playing: false,
            ppq: 960,
            playback_start_pos: 0,
            playback_pos_ticks: Arc::new(MIDITickAtomic::new(0)),
            start_time: Arc::new(Mutex::new(Instant::now())),
            start_pos_secs_from_ticks: 0.0f32,
            tempo_map: Arc::new(RwLock::new(Vec::new())),
            batch_size: Arc::new(Mutex::new(MidiEventBatchSize::BatchSize(4096))),

        }
    }

    pub fn navigate_to(&mut self, tick_pos: MIDITick) {
        self.playback_start_pos = tick_pos;
        self.playback_pos_ticks.store(tick_pos, Ordering::SeqCst);
    }

    pub fn stop(&mut self) {
        self.playback_pos_ticks.store(self.playback_start_pos, Ordering::SeqCst);
        self.stop_playback.store(true, Ordering::SeqCst);

        self.reset_events();
    }

    pub fn reset_stop(&self) {
        self.stop_playback.store(false, Ordering::SeqCst);
    }

    pub fn get_playback_ticks(&self) -> MIDITick {
        if !self.playing { self.playback_start_pos }
        else { 
            //self.playback_pos_ticks.load(Ordering::SeqCst)
            let tempo_map = self.tempo_map.read().unwrap();

            let elapsed = {
                let st = self.start_time.lock().unwrap();
                st.elapsed().as_secs_f32() + self.start_pos_secs_from_ticks
            };

            Self::secs_to_ticks_from_map(&tempo_map, self.ppq, elapsed) as MIDITick
        }
    }

    pub fn set_event_pool_size(&mut self, mut new_size: usize) {
        if new_size > 1000000 { new_size = 1000000; }
        if new_size < 100 { new_size = 100; }
        (self.tx, self.rx) = bounded(new_size);
        println!("New pool size: {}", new_size);
    }

    pub fn set_event_batch_size(&mut self, size: MidiEventBatchSize) {
        let mut batch_size = self.batch_size.lock().unwrap();
        *batch_size = size;
    }

    fn build_tempo_map(&self) -> Vec<(MIDITick, f32)> {
        let meta = self.meta_events.lock().unwrap();
        meta.iter()
            .filter(|m| m.event_type == MetaEventType::Tempo)
            .map(|m| (m.tick, bytes_as_tempo(&m.data)))
            .collect::<Vec<_>>()
    }

    fn ticks_to_secs_from_map(tempo_map: &[(MIDITick, f32)], ppq: u16, tick: f32) -> f32 {
        let mut last_tick = 0.0_f32;
        let mut last_tempo = if !tempo_map.is_empty() { tempo_map[0].1 } else { 120.0 }; // fallback
        let mut seconds = 0.0_f32;

        for &(ev_tick, ev_tempo) in tempo_map.iter().skip(1) {
            let ev_tick_f = ev_tick as f32;
            if ev_tick_f > tick { break; }
            let delta_ticks = ev_tick_f - last_tick;
            let us_per_qn = 60_000_000.0_f32 / last_tempo as f32;
            let sec_per_tick = us_per_qn / 1_000_000.0_f32 / ppq as f32;
            seconds += delta_ticks * sec_per_tick;
            last_tick = ev_tick_f;
            last_tempo = ev_tempo;
        }

        let delta_ticks = tick - last_tick;
        let us_per_qn = 60_000_000.0_f32 / last_tempo as f32;
        let sec_per_tick = us_per_qn / 1_000_000.0_f32 / ppq as f32;
        seconds + delta_ticks * sec_per_tick
    }

    fn secs_to_ticks_from_map(tempo_map: &[(MIDITick, f32)], ppq: u16, secs: f32) -> f32 {
        let mut last_tick = 0.0_f32;
        let mut last_tempo = if !tempo_map.is_empty() { tempo_map[0].1 } else { 120.0 }; // fallback
        let mut elapsed_secs = 0.0_f32;

        for &(ev_tick, ev_tempo) in tempo_map.iter().skip(1) {
            let ev_tick_f = ev_tick as f32;

            let delta_ticks = ev_tick_f - last_tick;
            let us_per_qn = 60_000_000.0_f32 / last_tempo as f32;
            let sec_per_tick = us_per_qn / 1_000_000.0_f32 / ppq as f32;
            let delta_secs = delta_ticks * sec_per_tick;
        
            if elapsed_secs + delta_secs > secs {
                let rem = secs - elapsed_secs;
                return last_tick + rem / sec_per_tick;
            }

            elapsed_secs += delta_secs;
            last_tick = ev_tick_f;
            last_tempo = ev_tempo;
        }

        let us_per_qn = 60_000_000.0_f32 / last_tempo;
        let sec_per_tick = us_per_qn / 1_000_000.0_f32 / ppq as f32;
        last_tick + (secs - elapsed_secs) / sec_per_tick
    }

    pub fn reset_events(&mut self) {
        // clear event pool
        while let Ok(_) = self.rx.try_recv() {}
        while let Ok(_) = self.notify_rx.try_recv() {}

        let mut devices = self.device.lock().unwrap();
        devices.send_event(&[0xB0, 0x7B, 0x00]).unwrap();
    }

    pub fn start_playback(&mut self) {
        self.reset_stop();
        let ppq = self.ppq;

        let notes = self.notes.clone();
        let channel_events = self.channel_events.clone();

        let stop_flag = self.stop_playback.clone();
        let playback_pos = self.playback_pos_ticks.clone();

        let tx = self.tx.clone();
        let notify_tx = self.notify_tx.clone();

        let tempo_map = self.tempo_map.clone();
        let start_time = self.start_time.clone();

        {
            let mut tempo_map = tempo_map.write().unwrap();
            *tempo_map = self.build_tempo_map();
        }

        let start_pos_secs_from_ticks = {
            let tempo_map = tempo_map.read().unwrap();
            self.start_pos_secs_from_ticks = Self::ticks_to_secs_from_map(&tempo_map, ppq, playback_pos.load(Ordering::SeqCst) as f32);
            self.start_pos_secs_from_ticks
        };

        thread::spawn(move || {
            {
                let mut st = start_time.lock().unwrap();
                *st = Instant::now();
            }

            let mut event_cursors = {
                let notes = notes.read().unwrap();
                let mut cursors = vec![0; notes.len()];

                for (trk, track) in notes.iter().enumerate() {
                    //for (chn, channel) in track.iter().enumerate() {
                    cursors[trk] = bin_search_notes_exact(&track, playback_pos.load(Ordering::SeqCst));
                    //}
                }
                cursors
            };

            let mut ch_event_cursors = {
                let ch_evs = channel_events.lock().unwrap();
                vec![0; ch_evs.len()]
            };

            //let mut scheduled_offs: BinaryHeap<Reverse<Scheduled>> = BinaryHeap::new();
            let mut scheduled_offs: ScheduledSequence = ScheduledSequence::new(128);

            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                //let elapsed = (*start_time).elapsed().as_secs_f32() + start_pos_secs_from_ticks;
                let elapsed = {
                    let st = start_time.lock().unwrap();
                    st.elapsed().as_secs_f32() + start_pos_secs_from_ticks
                };

                {
                    let tempo_map = tempo_map.read().unwrap();
                    playback_pos.store(Self::secs_to_ticks_from_map(&tempo_map, ppq, elapsed) as MIDITick, Ordering::SeqCst);
                }

                // first the control events / other stuff
                {
                    let channel_events = channel_events.lock().unwrap();
                    for (trk, track) in channel_events.iter().enumerate() {
                        // early break if stop flag is set
                        if stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        let cursor = &mut ch_event_cursors[trk];

                        while *cursor < track.len() {
                            if stop_flag.load(Ordering::SeqCst) {
                                break;
                            }

                            let event = &track[*cursor];

                            if playback_pos.load(Ordering::SeqCst) >= event.tick {
                                match event.event_type {
                                    ChannelEventType::Controller(controller, value) => {
                                        let _ = tx.try_send(MidiEvent::Control { channel: event.channel, controller: controller, value: value });
                                        let _ = notify_tx.try_send(());
                                    },
                                    ChannelEventType::PitchBend(lsb, msb) => {
                                        let _ = tx.try_send(MidiEvent::PitchBend { channel: event.channel, lsb: lsb, msb: msb });
                                        let _ = notify_tx.try_send(()); // pitchy bend notify
                                    }
                                    _ => {}
                                }
                                *cursor += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }

                while let Some(first) = scheduled_offs.peek() {
                    if first.time <= playback_pos.load(Ordering::SeqCst) {
                        let first = scheduled_offs.pop().unwrap();
                        let _ = tx.try_send(first.event); // wake up thread
                    } else {
                        break;
                    }
                }

                // then do notes
                {
                    let notes = notes.read().unwrap();
                    for (trk, track) in notes.iter().enumerate() {
                        if stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        //for (chn, channel) in track.iter().enumerate() {
                        //    if stop_flag.load(Ordering::SeqCst) {
                        //        break;
                        //    }
                            
                        //let cursor = &mut event_cursors[(trk << 4) | chn];
                        let cursor = &mut event_cursors[trk];

                        while *cursor < track.len() {
                            if stop_flag.load(Ordering::SeqCst) {
                                break;
                            }

                            let note = &track[*cursor];

                            if playback_pos.load(Ordering::SeqCst) >= note.start() {
                                if note.velocity() >= 20 {
                                    let _ = tx.try_send(MidiEvent::NoteOn { channel: note.channel(), key: note.key(), velocity: note.velocity() });
                                    let _ = notify_tx.try_send(()); // notify the playback thread of this note on event
                                    scheduled_offs.insert(Scheduled { time: note.end(), event: MidiEvent::NoteOff { channel: note.channel(), key: note.key(), velocity: note.velocity() } });
                                }
                                *cursor += 1;
                            } else {
                                break;
                            }
                        }
                        //}
                    }
                }

                // let the cpu sleep
                thread::sleep(Duration::from_millis(1));
            }

            // we stopped here
            let _ = notify_tx.try_send(());
        });
    }

    pub fn run_synth_loop(&self) {
        let device = self.device.clone();

        let rx = self.rx.clone();
        let notify_rx = self.notify_rx.clone();

        let stop_flag = self.stop_playback.clone();
        let batch_size = self.batch_size.clone();

        thread::spawn(move || {
            let send_ev = |device: &mut MutexGuard<'_, dyn MIDIAudioEngine + Send + 'static>, event: MidiEvent| {
                match event {
                    MidiEvent::NoteOn { channel, key, velocity } => {
                        device.send_event(&[0x90 | channel, key, velocity]).unwrap();
                    },
                    MidiEvent::NoteOff { channel, key, velocity } => {
                        device.send_event(&[0x80 | channel, key, velocity]).unwrap();
                    },
                    MidiEvent::Control { channel, controller, value } => {
                        device.send_event(&[0xB0 | channel, controller, value]).unwrap();
                    },
                    MidiEvent::PitchBend { channel, lsb, msb } => {
                        device.send_event(&[0xE0 | channel, lsb, msb]).unwrap();
                    }
                }
            };

            loop {
                crossbeam::select! {
                    recv(rx) -> msg => {
                        match msg {
                            Ok(first_event) => {
                                if stop_flag.load(Ordering::SeqCst) { break; }

                                let mut device = device.lock().unwrap();
                                
                                // always send the first event
                                send_ev(&mut device, first_event);

                                // now we can process the rest of the events
                                let mut evs_sent = 1;

                                let batch_size = batch_size.lock().unwrap();
                                
                                match *batch_size {
                                    MidiEventBatchSize::Unlimited => {
                                        while let Ok(event) = rx.try_recv() {
                                            if stop_flag.load(Ordering::SeqCst) { break; }
                                            send_ev(&mut device, event);
                                            evs_sent += 1;
                                        }
                                    },
                                    MidiEventBatchSize::BatchSize( max_batch) => {
                                        for _ in 1..max_batch {
                                            match rx.try_recv() {
                                                Ok(event) => {
                                                    if stop_flag.load(Ordering::SeqCst) { break; }
                                                    send_ev(&mut device, event);
                                                    evs_sent += 1;
                                                },
                                                Err(_) => {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }

                                println!("Events sent: {}", evs_sent);
                            },
                            Err(_) => break,
                        }
                    },
                    recv(notify_rx) -> _ => {
                        if stop_flag.load(Ordering::SeqCst) { break; }
                        continue;
                    }
                }
            }

            // stopped, so try to send a note off to all channels
            {
                let mut device = device.lock().unwrap();
                for note in 0..128 {
                    for chan in 0..16 {
                        device.send_event(&[0x80 | chan, note, 0x00]).unwrap();
                    }
                }

                // TODO: reset control events as well
            }
        });
    }

    pub fn toggle_playback(&mut self) {
        if !self.playing {
            self.start_playback();
            self.run_synth_loop();
            self.playing = true;
        } else {
            self.stop();
            self.playing = false;
        }
    }
}