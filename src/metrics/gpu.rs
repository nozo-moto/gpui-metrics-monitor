//! Windows GPU metric collection via native APIs only.
//!
//! Adapter identity and dedicated VRAM capacity come from DXGI
//! (`CreateDXGIFactory1` + `EnumAdapters1`, `DXGI_ADAPTER_DESC1`).
//! Utilization and dedicated memory usage come from PDH English counters:
//!
//! - `\GPU Engine(*)\Utilization Percentage`: process readings are summed
//!   for each engine and the busiest engine is reported per adapter, matched
//!   through the `luid_*` instance marker. This is a rate counter: values become
//!   valid on the second `sample()` call and represent the utilization
//!   averaged over the interval between consecutive samples.
//! - `\GPU Adapter Memory(*)\Dedicated Usage` — dedicated VRAM bytes in use,
//!   also matched to an adapter by LUID. This is a gauge, valid on first sample.
//!
//! No subprocess is spawned and no fabricated `0` is ever reported: readings
//! the OS does not supply reliably are `None`.
//!
//! Startup shows fewer readings than a warmed-up collector: utilization is
//! unavailable until PDH produces a second sample. `GPU Engine` counters
//! exist only when a process actually holds GPU work, so a fully idle GPU may
//! genuinely report `utilization_percent: None`.

use super::GpuSnapshot;

#[cfg(windows)]
use std::collections::HashMap;

#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
};
#[cfg(windows)]
use windows::Win32::System::Performance::{
    PDH_FMT, PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE,
    PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
#[cfg(windows)]
use windows::core::w;

#[cfg(windows)]
/// PDH return status codes (from pdh.h; not re-exported by windows-rs).
const PDH_MORE_DATA: u32 = 0x8000_07D2;
#[cfg(windows)]
const PDH_CSTATUS_VALID_DATA: u32 = 0x0000_0000;
#[cfg(windows)]
const PDH_CSTATUS_NEW_DATA: u32 = 0x0000_0001;
#[cfg(windows)]
const ERROR_SUCCESS: u32 = 0;

#[cfg(windows)]
type AdapterLuid = (u32, i32);

#[cfg(windows)]
struct PdhGpu {
    query: PDH_HQUERY,
    util_counter: PDH_HCOUNTER,
    mem_counter: PDH_HCOUNTER,
}

#[cfg(windows)]
impl Drop for PdhGpu {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

#[cfg(windows)]
impl PdhGpu {
    fn new() -> Result<Self, ()> {
        let mut query = PDH_HQUERY::default();
        let mut util_counter = PDH_HCOUNTER::default();
        let mut mem_counter = PDH_HCOUNTER::default();
        unsafe {
            if PdhOpenQueryW(None, 0, &mut query) != ERROR_SUCCESS {
                return Err(());
            }
            if PdhAddEnglishCounterW(
                query,
                w!(r"\GPU Engine(*)\Utilization Percentage"),
                0,
                &mut util_counter,
            ) != ERROR_SUCCESS
            {
                PdhCloseQuery(query);
                return Err(());
            }
            if PdhAddEnglishCounterW(
                query,
                w!(r"\GPU Adapter Memory(*)\Dedicated Usage"),
                0,
                &mut mem_counter,
            ) != ERROR_SUCCESS
            {
                PdhCloseQuery(query);
                return Err(());
            }
        }
        Ok(Self {
            query,
            util_counter,
            mem_counter,
        })
    }

    /// The return status is ignored on purpose: per-instance `CStatus`
    /// values are filtered when reading arrays, so partial collection still
    /// yields the readings it does have.
    fn collect(&self) -> bool {
        unsafe { PdhCollectQueryData(self.query) == ERROR_SUCCESS }
    }

    /// Sum process counters for each engine, then report the busiest engine.
    fn utilization_by_adapter(&self) -> Vec<(AdapterLuid, f64)> {
        let mut by_engine: HashMap<(AdapterLuid, String), f64> = HashMap::new();
        for (name, value) in unsafe { self.util_counter.formatted_array(PDH_FMT_DOUBLE) } {
            if let (Some(luid), Some(engine), Some(percent)) =
                (parse_luid(&name), parse_engine(&name), as_percent(value))
            {
                *by_engine.entry((luid, engine)).or_default() += percent;
            }
        }
        let mut by_adapter: HashMap<AdapterLuid, f64> = HashMap::new();
        for ((luid, _), percent) in by_engine {
            let busiest = by_adapter.entry(luid).or_default();
            *busiest = busiest.max(percent);
        }
        by_adapter.into_iter().collect()
    }

    /// `\GPU Adapter Memory(*)\Dedicated Usage` summed per adapter LUID.
    fn dedicated_usage_by_adapter(&self) -> Vec<(AdapterLuid, u64)> {
        let mut by_adapter: Vec<(AdapterLuid, u64)> = Vec::new();
        for (name, value) in unsafe { self.mem_counter.formatted_array(PDH_FMT_LARGE) } {
            if let (Some(luid), Some(bytes)) = (parse_luid(&name), as_bytes(value)) {
                match by_adapter.iter_mut().find(|(k, _)| *k == luid) {
                    Some((_, total)) => *total += bytes,
                    None => by_adapter.push((luid, bytes)),
                }
            }
        }
        by_adapter
    }
}

#[cfg(windows)]
trait FormattedArray {
    /// # Safety
    /// `self` must be a live counter handle obtained from the same PDH query
    /// as the caller's thread and must not have been closed.
    unsafe fn formatted_array(self, format: PDH_FMT) -> Vec<(String, PDH_FMT_COUNTERVALUE)>;
}

#[cfg(windows)]
impl FormattedArray for PDH_HCOUNTER {
    unsafe fn formatted_array(self, format: PDH_FMT) -> Vec<(String, PDH_FMT_COUNTERVALUE)> {
        unsafe {
            let mut size: u32 = 0;
            let mut count: u32 = 0;
            let status = PdhGetFormattedCounterArrayW(self, format, &mut size, &mut count, None);
            if status != PDH_MORE_DATA || size == 0 {
                return Vec::new();
            }
            let item_size = core::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let capacity = (size as usize).div_ceil(item_size);
            let mut buffer = Vec::<PDH_FMT_COUNTERVALUE_ITEM_W>::with_capacity(capacity);
            let status = PdhGetFormattedCounterArrayW(
                self,
                format,
                &mut size,
                &mut count,
                Some(buffer.as_mut_ptr()),
            );
            if status != ERROR_SUCCESS || count as usize > capacity {
                return Vec::new();
            }
            buffer.set_len(count as usize);
            buffer
                .iter()
                .map(|item| (item.szName.to_string().unwrap_or_default(), item.FmtValue))
                .collect()
        }
    }
}

#[cfg(windows)]
fn as_percent(value: PDH_FMT_COUNTERVALUE) -> Option<f64> {
    let readable = matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA);
    if !readable {
        return None;
    }
    let percent = unsafe { value.Anonymous.doubleValue };
    (percent.is_finite() && percent >= 0.0).then_some(percent)
}

#[cfg(windows)]
fn as_bytes(value: PDH_FMT_COUNTERVALUE) -> Option<u64> {
    let readable = matches!(value.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA);
    if !readable {
        return None;
    }
    let bytes = unsafe { value.Anonymous.largeValue };
    (bytes >= 0).then_some(bytes as u64)
}

#[cfg(windows)]
/// PDH instance strings embed the adapter LUID as `luid_0x<hipart>_0x<lowpart>_`.
fn parse_luid(instance: &str) -> Option<AdapterLuid> {
    let lower = instance.to_ascii_lowercase();
    let idx = lower.find("luid_")?;
    let mut parts = lower[idx + 5..].split('_');
    let high = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    let low = u32::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    Some((low, high as i32))
}

#[cfg(windows)]
fn parse_engine(instance: &str) -> Option<String> {
    let lower = instance.to_ascii_lowercase();
    let start = lower.find("_eng_")?;
    let engine = &lower[start + 1..];
    engine.contains("_engtype_").then(|| engine.to_owned())
}

#[cfg(windows)]
fn enumerate_adapters() -> Vec<(String, u64, AdapterLuid)> {
    let mut adapters = Vec::new();
    unsafe {
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            let mut index = 0u32;
            while let Ok(adapter) = factory.EnumAdapters1(index) {
                if let Ok(desc) = adapter.GetDesc1() {
                    let luid = (desc.AdapterLuid.LowPart, desc.AdapterLuid.HighPart);
                    // DXGI_ADAPTER_FLAG_SOFTWARE == 2: skip Microsoft Basic Render
                    // Driver and similar software devices; they have no real VRAM.
                    let is_software = desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0;
                    if !is_software {
                        adapters.push((
                            adapter_description(&desc.Description),
                            desc.DedicatedVideoMemory as u64,
                            luid,
                        ));
                    }
                }
                index += 1;
            }
        }
    }
    adapters
}

#[cfg(windows)]
fn adapter_description(chars: &[u16; 128]) -> String {
    let len = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..len])
}

pub struct GpuCollector {
    #[cfg(windows)]
    pdh: Option<PdhGpu>,
}

#[cfg(windows)]
impl Default for GpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl GpuCollector {
    pub fn new() -> Self {
        Self { pdh: None }
    }

    pub fn sample(&mut self) -> Vec<GpuSnapshot> {
        if self.pdh.is_none() {
            self.pdh = PdhGpu::new().ok();
        }
        let utilization: Vec<(AdapterLuid, f64)>;
        let usage: Vec<(AdapterLuid, u64)>;
        match self.pdh.as_ref() {
            Some(pdh) if pdh.collect() => {
                utilization = pdh.utilization_by_adapter();
                usage = pdh.dedicated_usage_by_adapter();
            }
            _ => {
                utilization = Vec::new();
                usage = Vec::new();
            }
        }
        enumerate_adapters()
            .into_iter()
            .map(|(name, dedicated_total_bytes, luid)| {
                let engine_total = utilization
                    .iter()
                    .find(|(k, _)| *k == luid)
                    .map(|(_, v)| *v);
                GpuSnapshot {
                    name,
                    utilization_percent: engine_total.map(|total| (total as f32).clamp(0.0, 100.0)),
                    dedicated_used_bytes: usage.iter().find(|(k, _)| *k == luid).map(|(_, v)| *v),
                    dedicated_total_bytes: Some(dedicated_total_bytes),
                }
            })
            .collect()
    }
}

#[cfg(not(windows))]
impl Default for GpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(windows))]
impl GpuCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn sample(&mut self) -> Vec<GpuSnapshot> {
        Vec::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn parse_luid_accepts_engine_instance_format() {
        assert_eq!(
            parse_luid("luid_0x00000000_0x0000C74B_phys_0"),
            Some((0x0000_C74B, 0))
        );
        assert_eq!(
            parse_luid("pid_8484_luid_0x00000000_0x00005A2F_eng_0_engtype_3D"),
            Some((0x0000_5A2F, 0))
        );
        assert_eq!(
            parse_luid("luid_0xFFFFFFFF_0x00000001_phys_0"),
            Some((0x0000_0001, -1))
        );
    }

    #[test]
    fn parse_luid_rejects_non_luid_instances() {
        assert_eq!(parse_luid("phys_0"), None);
        assert_eq!(parse_luid(""), None);
    }

    #[test]
    fn groups_processes_on_the_same_gpu_engine() {
        assert_eq!(
            parse_engine("pid_12_luid_0x0_0x1_phys_0_eng_0_engtype_3D"),
            parse_engine("pid_34_luid_0x0_0x1_phys_0_eng_0_engtype_3D")
        );
        assert_ne!(
            parse_engine("pid_12_luid_0x0_0x1_phys_0_eng_0_engtype_3D"),
            parse_engine("pid_12_luid_0x0_0x1_phys_0_eng_1_engtype_Copy")
        );
    }
}
