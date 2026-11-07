use super::SystemSnapshot;

pub struct SystemCollector;

impl SystemCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn sample(&mut self) -> SystemSnapshot {
        SystemSnapshot::default()
    }
}

