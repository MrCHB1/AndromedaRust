use std::time::Instant;

use sysinfo::{Pid, Process, System};

pub enum MemoryUnits {
    Bytes(u64),
    KiloBytes(f64),
    MegaBytes(f64),
    GigaBytes(f64),
    TeraBytes(f64) // rarely possible but here just in case
}

impl Default for MemoryUnits {
    fn default() -> Self {
        Self::Bytes(0)
    }
}

impl ToString for MemoryUnits {
    fn to_string(&self) -> String {
        let num: f64;

        let unit = match self {
            &Self::Bytes(b) => {
                num = b as f64;
                "bytes"
            },
            &Self::KiloBytes(b) => {
                num = b;
                "KB"
            },
            &Self::MegaBytes(b) => {
                num = b;
                "MB"
            },
            &Self::GigaBytes(b) => {
                num = b;
                "GB"
            },
            &Self::TeraBytes(b) => {
                num = b;
                "TB"
            }
        };

        format!("{num:.2} {unit}")
    }
}

pub struct SystemStats {
    sys: System,
    pid: Pid,
    refresh_timer: Instant,

    pub cpu_usage: f32,
    pub memory_usage: MemoryUnits,
    pub memory_pers: f32,
    pub total_memory: u64
}

impl Default for SystemStats {
    fn default() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let total_memory = sys.total_memory();
        Self {
            sys,
            refresh_timer: Instant::now(),
            pid: sysinfo::get_current_pid().unwrap(),

            cpu_usage: 0.0,
            memory_usage: MemoryUnits::default(),
            memory_pers: 0.0,
            total_memory
        }
    }
}

impl SystemStats {
    pub fn update(&mut self) {
        let rtimer = self.refresh_timer.elapsed().as_secs_f32();
        
        if rtimer >= 1.0 {
            self.sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[self.pid]), true);
            self.refresh_timer = Instant::now();
        } else {
            return;
        }

        // at this point, we can safely the stats now
        if let Some(process) = self.sys.process(self.pid) {
            let memory = process.memory();

            self.cpu_usage = process.cpu_usage() / self.sys.cpus().len() as f32;
            self.memory_usage = {
                if memory >= 1000000000000 {
                    MemoryUnits::TeraBytes(memory as f64 / 1000000000000.0f64)
                } else if memory >= 1000000000 {
                    MemoryUnits::GigaBytes(memory as f64 / 1000000000.0f64)
                } else if memory >= 1000000 {
                    MemoryUnits::MegaBytes(memory as f64 / 1000000.0f64)
                } else if memory >= 1000 {
                    MemoryUnits::KiloBytes(memory as f64 / 1000.0f64)
                } else {
                    MemoryUnits::Bytes(memory)
                }
            };
            self.memory_pers = (memory as f64 / self.total_memory as f64) as f32 * 100.0;
        }
    }
}