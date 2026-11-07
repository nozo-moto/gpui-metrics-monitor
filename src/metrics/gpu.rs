use super::GpuSnapshot;

pub struct GpuCollector;

impl GpuCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn sample(&mut self) -> Vec<GpuSnapshot> {
        Vec::new()
    }
}

