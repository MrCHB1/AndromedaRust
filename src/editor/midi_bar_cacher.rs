use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, RwLock}};

use crate::{editor::project::project_manager::ProjectManager, midi::events::meta_event::{MetaEvent, MetaEventType}};

pub struct BarCacher {
    // pub ppq: u16,
    pub project_manager: Arc<RwLock<ProjectManager>>,
    pub bar_cache: Vec<(u32, u32)>,
    // pub global_metas: Arc<RwLock<Vec<MetaEvent>>>,
    last_ts_index: usize
}

impl Default for BarCacher {
    fn default() -> Self {
        BarCacher::new(&Arc::new(RwLock::new(ProjectManager::new())))
    }
}

impl BarCacher {
    pub fn new(project_manger: &Arc<RwLock<ProjectManager>>) -> Self {
        Self {
            // ppq,
            bar_cache: Vec::new(),
            project_manager: project_manger.clone(),
            last_ts_index: 0
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
        let project_manager = self.project_manager.read().unwrap();
        let metas = project_manager.get_metas().read().unwrap();

        while self.bar_cache.len() <= target_bar {
            let start_tick = match self.bar_cache.last() {
                Some((s, l)) => s + l,
                None => 0u32
            };

            let (length, new_idx) = self.compute_bar_length_at(start_tick, &metas, self.last_ts_index, project_manager.get_ppq());
            self.last_ts_index = new_idx;
            self.bar_cache.push((start_tick, length));
        }
    }

    fn compute_bar_length_at(&self, start_tick: u32, metas: &Vec<MetaEvent>, search_idx: usize, ppq: u16) -> (u32, usize) {
        // Use cached index to avoid linear search from beginning
        let mut current_ts = None;
        let last_idx = search_idx;

        for i in search_idx..metas.len() {
            if metas[i].event_type == MetaEventType::TimeSignature {
                if metas[i].tick as u32 <= start_tick {
                    current_ts = Some(&metas[i])
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

        let ticks_per_beat = (ppq as u32) << 2;
        let nominal = (num * ticks_per_beat) >> den;

        let next_ts = metas[last_idx..]
            .iter()
            .find(|m| m.event_type == MetaEventType::TimeSignature && (m.tick as u32) > start_tick);
        
        let length = if let Some(next) = next_ts {
            let next_tick = next.tick as u32;
            if next_tick < start_tick + nominal {
                next_tick - start_tick
            } else {
                nominal
            }
        } else {
            nominal
        };

        (length, last_idx)
    }
}