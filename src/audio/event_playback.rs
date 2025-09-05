use std::{sync::{atomic::{AtomicBool, AtomicU32, Ordering}, Arc, Mutex}, thread::JoinHandle, time::{Duration, Instant}};

use crate::{audio::midi_devices::MIDIDevices, editor::project_data::bytes_as_tempo, midi::events::{channel_event::{ChannelEvent, ChannelEventType}, meta_event::{MetaEvent, MetaEventType}, note::Note}};
use crossbeam::channel::{bounded, Receiver, RecvTimeoutError, Sender};
use std::thread;

enum MidiEvent {
    NoteOn { channel: u8, key: u8, velocity: u8},
    NoteOff { channel: u8, key: u8, velocity: u8 },
    Control { channel: u8, controller: u8, value: u8 },
    PitchBend { channel: u8, lsb: u8, msb: u8 }
}

pub struct PlaybackManager {
    pub notes: Arc<Mutex<Vec<Vec<Vec<Note>>>>>,
    pub meta_events: Arc<Mutex<Vec<MetaEvent>>>,
    pub channel_events: Arc<Mutex<Vec<Vec<ChannelEvent>>>>,
    pub midi_devices: Arc<Mutex<MIDIDevices>>,
    pub ppq: u16,

    tx: Sender<MidiEvent>,
    rx: Receiver<MidiEvent>,
    stop_playback: Arc<AtomicBool>,
    pub playing: bool,
    pub playback_start_pos: u32,
    pub playback_pos_ticks: Arc<AtomicU32>,
}

impl PlaybackManager {
    pub fn new(midi_devices: Arc<Mutex<MIDIDevices>>, notes: Arc<Mutex<Vec<Vec<Vec<Note>>>>>, meta_events: Arc<Mutex<Vec<MetaEvent>>>, channel_events: Arc<Mutex<Vec<Vec<ChannelEvent>>>>) -> Self {
        let (tx, rx) = bounded(100000);
        Self {
            midi_devices, notes, meta_events, channel_events,
            //event_pool: Arc::new(Mutex::new(EventPool::new(100000))),
            tx, rx,
            stop_playback: Arc::new(AtomicBool::new(false)),
            playing: false,
            ppq: 960,
            playback_start_pos: 0,
            playback_pos_ticks: Arc::new(AtomicU32::new(0))
        }
    }

    pub fn navigate_to(&mut self, tick_pos: u32) {
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

    pub fn get_playback_ticks(&self) -> u32 {
        if !self.playing { self.playback_start_pos }
        else { self.playback_pos_ticks.load(Ordering::SeqCst) }
    }

    pub fn set_event_pool_size(&mut self, mut new_size: usize) {
        if new_size > 1000000 { new_size = 1000000; }
        if new_size < 100 { new_size = 100; }
        (self.tx, self.rx) = bounded(new_size);
        println!("New pool size: {}", new_size);
    }

    fn build_tempo_map(&self) -> Vec<(u64, f32)> {
        let meta = self.meta_events.lock().unwrap();
        meta.iter()
            .filter(|m| m.event_type == MetaEventType::Tempo)
            .map(|m| (m.tick as u64, bytes_as_tempo(&m.data)))
            .collect::<Vec<_>>()
    }

    fn ticks_to_secs_from_map(tempo_map: &[(u64, f32)], ppq: u16, tick: f32) -> f32 {
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

    fn secs_to_ticks_from_map(tempo_map: &[(u64, f32)], ppq: u16, secs: f32) -> f32 {
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

        let mut devices = self.midi_devices.lock().unwrap();
        devices.send_event(&[0xB0, 0x7B, 0x00]).unwrap();
    }

    pub fn start_playback(&mut self) {
        self.reset_stop();

        let notes = self.notes.clone();
        let channel_events = self.channel_events.clone();
        let tempo_map = self.build_tempo_map();

        let stop_flag = self.stop_playback.clone();
        let playback_pos = self.playback_pos_ticks.clone();
        let tx = self.tx.clone();
        let ppq = self.ppq;

        thread::spawn(move || {
            let start_time = Instant::now();

            let mut event_cursors = {
                let notes = notes.lock().unwrap();
                vec![0; notes.len() * 16]
            };

            let mut ch_event_cursors = {
                let ch_evs = channel_events.lock().unwrap();
                vec![0; ch_evs.len()]
            };

            let mut scheduled_offs: Vec<(f32, MidiEvent)> = Vec::new();

            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let elapsed = start_time.elapsed().as_secs_f32();
                playback_pos.store(Self::secs_to_ticks_from_map(&tempo_map, ppq, elapsed) as u32, Ordering::SeqCst);

                let mut i = 0;
                while i < scheduled_offs.len() {
                    if scheduled_offs[i].0 <= elapsed {
                        let (_t, ev) = scheduled_offs.swap_remove(i);
                        let _ = tx.try_send(ev);
                    } else {
                        i += 1;
                    }
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

                            let ev_time_secs = Self::ticks_to_secs_from_map(&tempo_map, ppq, event.tick as f32);
                            if elapsed >= ev_time_secs {
                                match event.event_type {
                                    ChannelEventType::Controller(controller, value) => {
                                        let _ = tx.try_send(MidiEvent::Control { channel: event.channel, controller: controller, value: value });
                                    },
                                    ChannelEventType::PitchBend(lsb, msb) => {
                                        let _ = tx.try_send(MidiEvent::PitchBend { channel: event.channel, lsb: lsb, msb: msb });
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
                // then do notes
                {
                    let notes = notes.lock().unwrap();
                    for (trk, track) in notes.iter().enumerate() {
                        if stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        for (chn, channel) in track.iter().enumerate() {
                            if stop_flag.load(Ordering::SeqCst) {
                                break;
                            }
                            let cursor = &mut event_cursors[trk * 16 + chn];

                            while *cursor < channel.len() {
                                if stop_flag.load(Ordering::SeqCst) {
                                    break;
                                }

                                let note = &channel[*cursor];

                                let note_start_secs = Self::ticks_to_secs_from_map(&tempo_map, ppq, note.start as f32);
                                if elapsed >= note_start_secs {
                                    if note.velocity >= 20 { 
                                        let _ = tx.try_send(MidiEvent::NoteOn { channel: chn as u8, key: note.key, velocity: note.velocity });
                                        
                                        let note_end_secs = Self::ticks_to_secs_from_map(&tempo_map, ppq, note.start as f32 + note.length as f32);

                                        // schedule the note off for sending it later
                                        scheduled_offs.push((note_end_secs, MidiEvent::NoteOff { channel: chn as u8, key: note.key, velocity: note.velocity }));
                                    }

                                    *cursor += 1;
                                } else {
                                    break; // note isnt ready yet
                                }
                            }
                        }
                    }
                }

                // let the cpu sleep
                thread::sleep(Duration::from_millis(1));
            }
        });
    }

    pub fn run_synth_loop(&self) {
        let midi_devices = self.midi_devices.clone();
        let rx = self.rx.clone();
        //let event_pool = self.event_pool.clone();

        let stop_flag = self.stop_playback.clone();
        thread::spawn(move || {
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        if stop_flag.load(Ordering::SeqCst) { break; }
                        let mut devices = midi_devices.lock().unwrap();
                        match event {
                            MidiEvent::NoteOn { channel, key, velocity } => {
                                devices.send_event(&[0x90 | channel, key, velocity]).unwrap();
                            },
                            MidiEvent::NoteOff { channel, key, velocity } => {
                                devices.send_event(&[0x80 | channel, key, velocity]).unwrap();
                            },
                            MidiEvent::Control { channel, controller, value } => {
                                devices.send_event(&[0xB0 | channel, controller, value]).unwrap();
                            },
                            MidiEvent::PitchBend { channel, lsb, msb } => {
                                devices.send_event(&[0xE0 | channel, lsb, msb]).unwrap();
                            }
                        }
                    },
                    Err(RecvTimeoutError::Timeout) => {
                        if stop_flag.load(Ordering::SeqCst) { break; }
                    },
                    Err(RecvTimeoutError::Disconnected) => break
                }
            }
            /*while let Ok(event) = rx.recv() {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let mut devices = midi_devices.lock().unwrap();
                match event {
                    MidiEvent::NoteOn { channel, key, velocity } => {
                        devices.send_event(&[0x90 | channel, key, velocity]).unwrap();
                    },
                    MidiEvent::NoteOff { channel, key } => {
                        devices.send_event(&[0x80 | channel, key]).unwrap();
                    }
                }
            }*/
        });
    }

    pub fn toggle_playback(&mut self) {
        //if !self.stop_playback.load(Ordering::SeqCst) {
        if !self.playing {
            self.start_playback();
            self.run_synth_loop();
            self.playing = true;
        } else {
            self.stop();
            self.playing = false;
        }
        //} else {
        //}
    }
}