//! Shared data contract for the Windows metric collectors and GPUI dashboard.
//! Counters represent rates over the collector's preceding sample interval.

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub cpu_percent: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub networks: Vec<NetworkSnapshot>,
    pub disks: Vec<DiskSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkSnapshot {
    pub name: String,
    pub received_bytes_per_sec: Option<f64>,
    pub transmitted_bytes_per_sec: Option<f64>,
    pub received_total_bytes: Option<u64>,
    pub transmitted_total_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct DiskSnapshot {
    pub name: String,
    pub mount_point: String,
    pub used_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub read_bytes_per_sec: Option<f64>,
    pub written_bytes_per_sec: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct GpuSnapshot {
    pub name: String,
    pub utilization_percent: Option<f32>,
    pub dedicated_used_bytes: Option<u64>,
    pub dedicated_total_bytes: Option<u64>,
}

pub mod gpu;
pub mod system;

