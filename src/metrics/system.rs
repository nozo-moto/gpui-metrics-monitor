use std::{
    collections::HashMap,
    ffi::OsStr,
    path::Path,
    time::{Duration, Instant},
};

use super::{DiskSnapshot, NetworkSnapshot, SystemSnapshot};
use sysinfo::{Disks, Networks, System};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DiskId {
    id: Option<u64>,
    name: String,
    mount_point: String,
    file_system: String,
}

fn os_str_to_string(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn path_to_string(value: &Path) -> String {
    value.to_string_lossy().into_owned()
}

fn rate(current: u64, previous: u64, elapsed: Duration) -> Option<f64> {
    if elapsed.is_zero() {
        return None;
    }

    current
        .checked_sub(previous)
        .map(|delta| delta as f64 / elapsed.as_secs_f64())
}

pub struct SystemCollector {
    system: Option<System>,
    networks: Option<Networks>,
    disks: Option<Disks>,
    previous_networks: HashMap<String, (u64, u64)>,
    previous_disks: HashMap<DiskId, (u64, u64)>,
    previous_sample_time: Option<Instant>,
    previous_cpu_time: Option<Instant>,
}

impl SystemCollector {
    pub fn new() -> Self {
        Self {
            system: System::new().ok(),
            networks: Networks::new_with_refreshed_list().ok(),
            disks: Disks::new_with_refreshed_list().ok(),
            previous_networks: HashMap::new(),
            previous_disks: HashMap::new(),
            previous_sample_time: None,
            previous_cpu_time: None,
        }
    }

    pub fn sample(&mut self) -> SystemSnapshot {
        let sample_started = Instant::now();
        let elapsed = self
            .previous_sample_time
            .and_then(|previous| sample_started.checked_duration_since(previous));

        let (cpu_percent, memory_used_bytes, memory_total_bytes) =
            if let Some(system) = &mut self.system {
                system.refresh_cpu_usage();
                system.refresh_memory();

                let cpu_percent = match self.previous_cpu_time {
                    Some(previous)
                        if sample_started.checked_duration_since(previous).is_some_and(
                            |duration| duration >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL,
                        ) =>
                    {
                        let usage = system.global_cpu_usage();
                        usage.is_finite().then_some(usage)
                    }
                    _ => None,
                };
                self.previous_cpu_time = Some(sample_started);

                (
                    cpu_percent,
                    Some(system.used_memory()),
                    Some(system.total_memory()),
                )
            } else {
                (None, None, None)
            };

        let networks = self.sample_networks(elapsed);
        let disks = self.sample_disks(elapsed);

        self.previous_sample_time = Some(sample_started);

        SystemSnapshot {
            cpu_percent,
            memory_used_bytes,
            memory_total_bytes,
            networks,
            disks,
        }
    }

    fn sample_networks(&mut self, elapsed: Option<Duration>) -> Vec<NetworkSnapshot> {
        let Some(networks) = &mut self.networks else {
            self.previous_networks.clear();
            return Vec::new();
        };

        // Refresh the interface list and remove disappeared interfaces.
        networks.refresh(true);

        let mut current = HashMap::new();
        let mut snapshots = Vec::with_capacity(networks.len());

        for (name, counters) in networks.iter() {
            let name = os_str_to_string(name);
            let received = counters.total_received();
            let transmitted = counters.total_transmitted();
            let previous = self.previous_networks.get(&name);
            let (received_rate, transmitted_rate) = match (previous, elapsed) {
                (Some(&(previous_received, previous_transmitted)), Some(elapsed)) => (
                    rate(received, previous_received, elapsed),
                    rate(transmitted, previous_transmitted, elapsed),
                ),
                _ => (None, None),
            };

            current.insert(name.clone(), (received, transmitted));
            snapshots.push(NetworkSnapshot {
                name,
                received_bytes_per_sec: received_rate,
                transmitted_bytes_per_sec: transmitted_rate,
                received_total_bytes: Some(received),
                transmitted_total_bytes: Some(transmitted),
            });
        }

        self.previous_networks = current;
        snapshots
    }

    fn sample_disks(&mut self, elapsed: Option<Duration>) -> Vec<DiskSnapshot> {
        let Some(disks) = &mut self.disks else {
            self.previous_disks.clear();
            return Vec::new();
        };

        // Re-enumerate devices so new disks are seen and removed disks are dropped.
        disks.refresh(true);

        let mut current = HashMap::new();
        let mut snapshots = Vec::with_capacity(disks.len());

        for disk in &mut *disks {
            let name = os_str_to_string(disk.name());
            let mount_point = path_to_string(disk.mount_point());
            let file_system = os_str_to_string(disk.file_system());
            let id = DiskId {
                id: disk.id(),
                name: name.clone(),
                mount_point: mount_point.clone(),
                file_system,
            };
            let usage = disk.usage();
            let read = usage.total_read_bytes;
            let written = usage.total_written_bytes;
            let previous = self.previous_disks.get(&id);
            let (read_rate, written_rate) = match (previous, elapsed) {
                (Some(&(previous_read, previous_written)), Some(elapsed)) => (
                    rate(read, previous_read, elapsed),
                    rate(written, previous_written, elapsed),
                ),
                _ => (None, None),
            };

            current.insert(id, (read, written));
            let total_bytes = disk.total_space();
            let used_bytes = total_bytes.checked_sub(disk.available_space());
            snapshots.push(DiskSnapshot {
                name,
                mount_point,
                used_bytes,
                total_bytes: Some(total_bytes),
                read_bytes_per_sec: read_rate,
                written_bytes_per_sec: written_rate,
            });
        }

        self.previous_disks = current;
        snapshots
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}
