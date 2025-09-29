use std::sync::{Arc, Mutex, RwLock};

use crate::midi::events::meta_event::{MetaEvent, MetaEventType};

pub struct BarCacher {
    pub ppq: u16,
    pub bar_cache: Vec<(u32, u32)>,
    pub global_metas: Arc<RwLock<Vec<MetaEvent>>>,
}

impl Default for BarCacher {
    fn default() -> Self {
        BarCacher::new(960, &Arc::new(RwLock::new(Vec::new())))
    }
}

impl BarCacher {
    pub fn new(ppq: u16, metas: &Arc<RwLock<Vec<MetaEvent>>>) -> Self {
        Self {
            ppq,
            bar_cache: Vec::new(),
            global_metas: metas.clone()
        }
    }

    pub fn clear_cache(&mut self) {
        self.bar_cache.clear();
    }

    pub fn get_bar_interval(&mut self, bar_num: usize) -> (u32, u32) {
        if bar_num < self.bar_cache.len() {
            return self.bar_cache[bar_num];
        }

        self.validate_bars_until(bar_num);

        self.bar_cache[bar_num]
    }

    fn validate_bars_until(&mut self, target_bar: usize) {
        let metas = self.global_metas.read().unwrap();

        while self.bar_cache.len() <= target_bar {
            let start_tick = match self.bar_cache.last() {
                Some((s, l)) => s + l,
                None => 0u32
            };

            let length = self.compute_bar_length_at(start_tick, &metas);
            self.bar_cache.push((start_tick, length));
        }
    }

    fn compute_bar_length_at(&self, start_tick: u32, metas: &Vec<MetaEvent>) -> u32 {
        let mut current_ts = None;
        for meta in metas.iter() {
            if meta.event_type == MetaEventType::TimeSignature {
                if meta.tick as u32 <= start_tick {
                    current_ts = Some(meta)
                } else {
                    break;
                }
            }
        }

        let (num, den) = if let Some(ts) = current_ts {
            (ts.data[0] as u32, ts.data[1] as u32)
        } else {
            (4, 2)
        };

        let ticks_per_beat = (self.ppq as u32) << 2;
        let nominal = (num * ticks_per_beat) >> den;

        let next_ts = metas.iter()
            .find(|m| m.event_type == MetaEventType::TimeSignature && (m.tick as u32) > start_tick);
        if let Some(next) = next_ts {
            let next_tick = next.tick as u32;
            if next_tick < start_tick + nominal {
                return next_tick - start_tick;
            }
        }

        nominal
    }
}