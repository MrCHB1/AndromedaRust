use std::time::Instant;

use sysinfo::{Pid, Process, System};

pub enum MemoryUnits {
    Bytes(u64),
    KiloBytes(f64),
    MegaBytes(f64),
    GigaBytes(f64),
    TeraBytes(f64) // rarely possible but here just in case
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
    refresh_timer: Instant
}

impl Default for SystemStats {
    fn default() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            sys,
            refresh_timer: Instant::now(),
            pid: sysinfo::get_current_pid().unwrap()
        }
    }
}

impl SystemStats {
    /// Returns CPU Usage (%), Ram Usage (bytes, KB, MB, GB, TB), % of RAM used, and if the stats has updated
    pub fn get_stats(&mut self) -> Option<(f32, MemoryUnits, f32, bool)> {
        let rtimer = self.refresh_timer.elapsed().as_secs_f64();

        let mut updated = false;
        if rtimer >= 0.5 {
            self.sys.refresh_all();
            self.refresh_timer = Instant::now();
            updated = true;
        }

        if let Some(process) = self.sys.process(self.pid) {
            Some((
                process.cpu_usage() / self.sys.cpus().len() as f32,
                {
                    let memory = process.memory();
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
                },
                (process.memory() as f64 / self.sys.total_memory() as f64) as f32 * 100.0,
                updated
            ))
        } else {
            None
        }
    }
}