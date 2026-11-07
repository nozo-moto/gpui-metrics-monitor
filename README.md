# GPUI Metrics Monitor

A native Windows dashboard for CPU, memory, GPU, network, and disk metrics.

## Architecture

- `src/metrics/mod.rs` owns the data contract. Optional readings are `None` when the operating system cannot supply a reliable value.
- `src/metrics/system.rs` collects CPU, memory, network interface, and disk readings. Rates are calculated from counter deltas and elapsed monotonic time; the first rate sample may be unavailable.
- `src/metrics/gpu.rs` collects GPU readings through Windows APIs. Display an unavailable reading instead of a made-up zero.
- `src/main.rs` and `src/ui.rs` run the GPUI desktop dashboard. Sampling should not block the UI event loop.

The initial files are an integration contract. The collectors and dashboard are implemented in separate Orca worktrees.
