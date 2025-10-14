use std::sync::{Arc, RwLock};
use crate::midi::events::meta_event::*;
use crate::editor::{
    project_data::bytes_as_tempo,
    util::MIDITick
};

pub struct TempoMap {
    pub meta_events: Arc<RwLock<Vec<MetaEvent>>>,
    tempo_map: Vec<(MIDITick, f32)>
}

impl Default for TempoMap {
    fn default() -> Self {
        Self {
            meta_events: Arc::new(RwLock::new(Vec::new())),
            tempo_map: Vec::new()
        }
    }
}

impl TempoMap {
    pub fn rebuild_tempo_map(&mut self) {
        let meta = self.meta_events.read().unwrap();
        self.tempo_map = meta.iter()
            .filter(|m| m.event_type == MetaEventType::Tempo)
            .map(|m| (m.tick, bytes_as_tempo(&m.data)))
            .collect::<Vec<_>>()
    }

    pub fn ticks_to_secs_from_map(&self, ppq: u16, tick: f32) -> f32 {
        let mut last_tick = 0.0_f32;
        let mut last_tempo = if !self.tempo_map.is_empty() { self.tempo_map[0].1 } else { 120.0 }; // fallback
        let mut seconds = 0.0_f32;

        for &(ev_tick, ev_tempo) in self.tempo_map.iter().skip(1) {
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

    pub fn secs_to_ticks_from_map(&self, ppq: u16, secs: f32) -> f32 {
        let mut last_tick = 0.0_f32;
        let mut last_tempo = if !self.tempo_map.is_empty() { self.tempo_map[0].1 } else { 120.0 }; // fallback
        let mut elapsed_secs = 0.0_f32;

        for &(ev_tick, ev_tempo) in self.tempo_map.iter().skip(1) {
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
}