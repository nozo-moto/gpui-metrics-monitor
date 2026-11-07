# GPUI Metrics Monitor

A native Windows dashboard for CPU, memory, GPU, network, and disk metrics, built with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

## Run

Use a current stable Rust toolchain for Windows MSVC:

```powershell
cargo run
```

The dashboard refreshes once per second and keeps 60 seconds of in-memory history for CPU, memory, GPU utilization, aggregate network throughput, and aggregate disk throughput. Its collector runs on a separate thread from the GPUI event loop. A reading is shown as unavailable when Windows does not supply a reliable value. Rate counters need two samples, so a few readings are unavailable during the first second.

History is drawn as fixed-width line graphs. CPU and GPU use a 0–100% vertical range; memory uses 0 to installed RAM. Network and disk throughput use fixed **display ranges** of 0–10 MiB/s and 0–100 MiB/s respectively. These ranges are not hardware capacity estimates. Readings above them are clipped on the plot and flagged as over range. Missing readings leave gaps in the line.

## Data sources

- CPU, memory, network interfaces, and disks: `sysinfo`.
- GPU adapter identity and dedicated memory capacity: Windows DXGI.
- GPU utilization and dedicated memory usage: Windows Performance Data Helper English counters. Utilization is the busiest GPU engine after summing process counters for that engine. It may be unavailable on the first sample or on systems without the relevant counters.

Network and disk throughput is calculated from counter deltas and monotonic elapsed time. Counter resets and disappeared devices are handled without showing negative or stale rates. Disk throughput represents the device counter reported by `sysinfo`, while capacity is reported for its mounted volume.

## Checks

```powershell
cargo fmt --check
cargo check
cargo test
```
