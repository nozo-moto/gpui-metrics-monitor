use crate::metrics::{DiskSnapshot, GpuSnapshot, NetworkSnapshot, SystemSnapshot};
use gpui::*;
use std::collections::VecDeque;

const PAGE: u32 = 0x0b1220;
const PANEL: u32 = 0x121d2e;
const PANEL_EDGE: u32 = 0x26364c;
const TEXT: u32 = 0xe5edf7;
const MUTED: u32 = 0x91a2b8;
const ACCENT: u32 = 0x62d6bd;
const BLUE: u32 = 0x7eb7ff;
const GRAPH_SAMPLES: usize = 60;

#[derive(Default)]
struct MetricHistory {
    cpu: VecDeque<Option<f64>>,
    memory: VecDeque<Option<f64>>,
    gpu: VecDeque<Option<f64>>,
    network: VecDeque<Option<f64>>,
    disk: VecDeque<Option<f64>>,
}

impl MetricHistory {
    fn push(history: &mut VecDeque<Option<f64>>, value: Option<f64>) {
        if history.len() == GRAPH_SAMPLES {
            history.pop_front();
        }
        history.push_back(value.filter(|sample| sample.is_finite() && *sample >= 0.));
    }
}

#[derive(Default)]
pub struct Dashboard {
    system: SystemSnapshot,
    gpus: Vec<GpuSnapshot>,
    history: MetricHistory,
}

impl Dashboard {
    pub fn update(&mut self, system: SystemSnapshot, gpus: Vec<GpuSnapshot>) {
        MetricHistory::push(
            &mut self.history.cpu,
            system.cpu_percent.map(|value| value as f64),
        );
        let memory = match (system.memory_used_bytes, system.memory_total_bytes) {
            (Some(used), Some(total)) if total > 0 => Some(percent(used, total)),
            _ => None,
        };
        MetricHistory::push(&mut self.history.memory, memory);
        let gpu_average = if gpus.is_empty() {
            None
        } else {
            gpus.iter()
                .try_fold((0_f64, 0_usize), |(sum, count), gpu| {
                    let value = gpu.utilization_percent? as f64;
                    (value.is_finite() && value >= 0.).then_some((sum + value, count + 1))
                })
                .map(|(sum, count)| sum / count as f64)
        };
        MetricHistory::push(&mut self.history.gpu, gpu_average);
        MetricHistory::push(
            &mut self.history.network,
            aggregate_throughput(system.networks.iter().map(|network| {
                (
                    network.received_bytes_per_sec,
                    network.transmitted_bytes_per_sec,
                )
            })),
        );
        MetricHistory::push(
            &mut self.history.disk,
            aggregate_throughput(
                system
                    .disks
                    .iter()
                    .map(|disk| (disk.read_bytes_per_sec, disk.written_bytes_per_sec)),
            ),
        );
        self.system = system;
        self.gpus = gpus;
    }

    fn history_graph(
        &self,
        title: &'static str,
        samples: &VecDeque<Option<f64>>,
        color: u32,
        fixed_max: Option<f64>,
        unit: &'static str,
    ) -> Div {
        let peak = fixed_max.unwrap_or_else(|| {
            samples
                .iter()
                .filter_map(|sample| *sample)
                .fold(0_f64, f64::max)
                .max(1.)
        });
        let mut bars = div().flex().items_end().gap(px(1.)).h(px(34.)).w_full();
        for sample in samples {
            let height = sample.map(|value| (value / peak * 34.).clamp(1., 34.));
            let bar = match height {
                Some(height) => div()
                    .flex_1()
                    .h(px(height as f32))
                    .rounded_sm()
                    .bg(rgb(color)),
                None => div().flex_1().h(px(1.)),
            };
            bars = bars.child(bar);
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(div().text_xs().text_color(rgb(MUTED)).child(title))
                    .child(div().text_xs().text_color(rgb(MUTED)).child(unit)),
            )
            .child(bars)
    }

    fn metric_card(
        &self,
        title: &'static str,
        value: String,
        detail: String,
        accent: u32,
        graph: Div,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(260.))
            .h(px(188.))
            .flex()
            .flex_col()
            .justify_between()
            .p_5()
            .rounded_xl()
            .border_1()
            .border_color(rgb(PANEL_EDGE))
            .bg(rgb(PANEL))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MUTED))
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(accent))
                            .child(value),
                    )
                    .child(div().text_sm().text_color(rgb(MUTED)).child(detail)),
            )
            .child(graph)
    }

    fn list_panel(
        &self,
        title: &'static str,
        entries: Vec<(String, String, String)>,
        empty_label: &'static str,
    ) -> Div {
        let mut panel = div()
            .flex_1()
            .min_w(px(320.))
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .rounded_xl()
            .border_1()
            .border_color(rgb(PANEL_EDGE))
            .bg(rgb(PANEL))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MUTED))
                    .child(title),
            );

        let graph = match title {
            "GPUS" => Some(self.history_graph(
                "Average GPU utilization · 60 s",
                &self.history.gpu,
                ACCENT,
                Some(100.),
                "%",
            )),
            "NETWORK" => Some(self.history_graph(
                "Aggregate RX + TX · 60 s",
                &self.history.network,
                BLUE,
                None,
                "KiB/s",
            )),
            "DISKS" => Some(self.history_graph(
                "Aggregate read + write · 60 s",
                &self.history.disk,
                ACCENT,
                None,
                "KiB/s",
            )),
            _ => None,
        };
        if let Some(graph) = graph {
            panel = panel.child(graph);
        }

        if entries.is_empty() {
            panel = panel.child(div().text_sm().text_color(rgb(MUTED)).child(empty_label));
        } else {
            for (name, first, second) in entries {
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(TEXT))
                                .truncate()
                                .child(name),
                        )
                        .child(div().text_sm().text_color(rgb(MUTED)).child(first))
                        .child(div().text_sm().text_color(rgb(MUTED)).child(second)),
                );
            }
        }
        panel
    }

    fn network_entries(&self) -> Vec<(String, String, String)> {
        self.system
            .networks
            .iter()
            .map(|network: &NetworkSnapshot| {
                (
                    network.name.clone(),
                    format!(
                        "RX {}  |  TX {}",
                        format_rate(network.received_bytes_per_sec),
                        format_rate(network.transmitted_bytes_per_sec)
                    ),
                    format!(
                        "Received {}  |  sent {}",
                        format_bytes(network.received_total_bytes),
                        format_bytes(network.transmitted_total_bytes)
                    ),
                )
            })
            .collect()
    }

    fn gpu_entries(&self) -> Vec<(String, String, String)> {
        self.gpus
            .iter()
            .map(|gpu| {
                let utilization = gpu
                    .utilization_percent
                    .filter(|value| value.is_finite() && *value >= 0.)
                    .map(|value| format!("Utilization {:.1}%", value.min(100.)))
                    .unwrap_or_else(|| "Utilization unavailable".to_owned());
                let memory = match (gpu.dedicated_used_bytes, gpu.dedicated_total_bytes) {
                    (Some(used), Some(total)) => format!(
                        "Dedicated VRAM {} / {}",
                        format_bytes(Some(used)),
                        format_bytes(Some(total))
                    ),
                    _ => "Dedicated VRAM unavailable".to_owned(),
                };
                (gpu.name.clone(), utilization, memory)
            })
            .collect()
    }

    fn disk_entries(&self) -> Vec<(String, String, String)> {
        self.system
            .disks
            .iter()
            .map(|disk: &DiskSnapshot| {
                let label = if disk.mount_point.is_empty() {
                    disk.name.clone()
                } else {
                    format!("{}  |  {}", disk.name, disk.mount_point)
                };
                let capacity = match (disk.used_bytes, disk.total_bytes) {
                    (Some(used), Some(total)) => {
                        format!(
                            "{} / {} used",
                            format_bytes(Some(used)),
                            format_bytes(Some(total))
                        )
                    }
                    _ => "Capacity unavailable".to_owned(),
                };
                (
                    label,
                    capacity,
                    format!(
                        "Read {}  |  write {}",
                        format_rate(disk.read_bytes_per_sec),
                        format_rate(disk.written_bytes_per_sec)
                    ),
                )
            })
            .collect()
    }
}

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cpu = self
            .system
            .cpu_percent
            .filter(|value| value.is_finite() && *value >= 0.)
            .map(|value| format!("{value:.1}%"))
            .unwrap_or_else(|| "Unavailable".to_owned());
        let memory = match (
            self.system.memory_used_bytes,
            self.system.memory_total_bytes,
        ) {
            (Some(used), Some(total)) if total > 0 => format!("{:.1}%", percent(used, total)),
            _ => "Unavailable".to_owned(),
        };
        let memory_detail = match (
            self.system.memory_used_bytes,
            self.system.memory_total_bytes,
        ) {
            (Some(used), Some(total)) if total > 0 => {
                format!(
                    "{} / {}",
                    format_bytes(Some(used)),
                    format_bytes(Some(total))
                )
            }
            _ => "Memory usage unavailable".to_owned(),
        };
        let gpu_entries = self.gpu_entries();
        let network_entries = self.network_entries();
        let disk_entries = self.disk_entries();

        div()
            .id("dashboard-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_5()
            .p_7()
            .bg(rgb(PAGE))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(TEXT))
                            .child("System dashboard"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("Live hardware metrics - refreshes every second"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.metric_card(
                        "CPU",
                        cpu,
                        "Total processor utilization".to_owned(),
                        ACCENT,
                        self.history_graph(
                            "History · 60 s",
                            &self.history.cpu,
                            ACCENT,
                            Some(100.),
                            "%",
                        ),
                    ))
                    .child(self.metric_card(
                        "MEMORY",
                        memory,
                        memory_detail,
                        BLUE,
                        self.history_graph(
                            "History · 60 s",
                            &self.history.memory,
                            BLUE,
                            Some(100.),
                            "%",
                        ),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.list_panel("GPUS", gpu_entries, "No GPU readings available"))
                    .child(self.list_panel(
                        "NETWORK",
                        network_entries,
                        "No network readings available",
                    )),
            )
            .child(self.list_panel("DISKS", disk_entries, "No disk readings available"))
    }
}

fn format_rate(value: Option<f64>) -> String {
    value
        .filter(|bytes_per_sec| bytes_per_sec.is_finite() && *bytes_per_sec >= 0.)
        .map(|bytes_per_sec| format!("{}/s", format_bytes(Some(bytes_per_sec as u64))))
        .unwrap_or_else(|| "Unavailable".to_owned())
}

fn format_bytes(value: Option<u64>) -> String {
    let Some(value) = value else {
        return "Unavailable".to_owned();
    };
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut amount = value as f64;
    let mut unit = 0;
    while amount >= 1024. && unit < UNITS.len() - 1 {
        amount /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{amount:.1} {}", UNITS[unit])
    }
}

fn percent(used: u64, total: u64) -> f64 {
    (used as f64 / total as f64 * 100.).min(100.)
}

fn aggregate_throughput(values: impl Iterator<Item = (Option<f64>, Option<f64>)>) -> Option<f64> {
    let mut total = 0.;
    let mut found = false;
    for (first, second) in values {
        let (Some(first), Some(second)) = (first, second) else {
            return None;
        };
        if !first.is_finite() || !second.is_finite() || first < 0. || second < 0. {
            return None;
        }
        total += first + second;
        found = true;
    }
    (found && total.is_finite()).then_some(total / 1024.)
}
