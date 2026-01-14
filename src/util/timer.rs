use std::time::Instant;

pub struct Timer {
    instant: Instant,
    last_time: f32
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            instant: Instant::now(),
            last_time: 0.0
        }
    }
}

impl Timer {
    pub fn start(&mut self) {
        self.instant = Instant::now()
    }

    #[inline(always)]
    pub fn elapsed(&self) -> f32 {
        self.instant.elapsed().as_secs_f32()
    }

    pub fn get_delta_time(&mut self) -> f32 {
        let now = self.elapsed();
        let delta = now - self.last_time;
        self.last_time = now;
        delta
    }
}