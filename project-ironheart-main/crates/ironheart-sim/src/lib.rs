#[derive(Debug, Clone, PartialEq)]
pub struct SimSnapshot {
    pub date_label: String,
    pub map_mode: String,
    pub zoom: f32,
}

pub struct SimRuntime {
    hour: u32,
}

impl SimRuntime {
    pub fn new() -> Self {
        Self { hour: 0 }
    }

    pub fn tick_hour(&mut self) {
        self.hour = self.hour.saturating_add(1);
    }

    pub fn snapshot(&self) -> SimSnapshot {
        SimSnapshot {
            date_label: format!("1936-01-01 {:02}:00", self.hour % 24),
            map_mode: "Political".to_string(),
            zoom: 0.35,
        }
    }
}

impl Default for SimRuntime {
    fn default() -> Self {
        Self::new()
    }
}
