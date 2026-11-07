use crate::metrics::{DiskSnapshot, GpuSnapshot, NetworkSnapshot, SystemSnapshot};
use gpui::*;

const PAGE: u32 = 0x0b1220;
const PANEL: u32 = 0x121d2e;
const PANEL_EDGE: u32 = 0x26364c;
const TEXT: u32 = 0xe5edf7;
const MUTED: u32 = 0x91a2b8;
const ACCENT: u32 = 0x62d6bd;
const BLUE: u32 = 0x7eb7ff;

#[derive(Default)]
pub struct Dashboard {
    system: SystemSnapshot,
    gpus: Vec<GpuSnapshot>,
}

impl Dashboard {
    pub fn update(&mut self, system: SystemSnapshot, gpus: Vec<GpuSnapshot>) {
        self.system = system;
        self.gpus = gpus;
    }

    fn metric_card(&self, title: &'static str, value: String, detail: String, accent: u32) -> Div {
        div()
            .flex_1()
            .min_w(px(260.))
            .h(px(164.))
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
                    .map(|value| format!("Utilization {value:.1}%"))
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
            .map(|value| format!("{value:.1}%"))
            .unwrap_or_else(|| "Unavailable".to_owned());
        let memory = match (
            self.system.memory_used_bytes,
            self.system.memory_total_bytes,
        ) {
            (Some(used), Some(total)) => format!("{:.1}%", percent(used, total)),
            _ => "Unavailable".to_owned(),
        };
        let memory_detail = match (
            self.system.memory_used_bytes,
            self.system.memory_total_bytes,
        ) {
            (Some(used), Some(total)) => {
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
                    ))
                    .child(self.metric_card("MEMORY", memory, memory_detail, BLUE)),
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
        .map(|bytes_per_sec| format!("{}/s", format_bytes(Some(bytes_per_sec.max(0.) as u64))))
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
    if total == 0 {
        0.
    } else {
        (used as f64 / total as f64 * 100.).min(100.)
    }
}
