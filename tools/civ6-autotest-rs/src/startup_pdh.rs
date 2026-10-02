//! Read-only native WDDM adapter enumeration and three PDH GPU Engine samples.
//! Runs in the bounded diagnostic worker, never in the driver's input thread.
use crate::{startup_idle::*, Error, Result};
use std::{
    ptr,
    time::{Duration, Instant},
};
use windows_sys::{Wdk::Graphics::Direct3D::*, Win32::System::Performance::*};

fn adapters() -> Result<(Vec<AdapterCoverage>, Vec<String>)> {
    struct Adapters(Vec<D3DKMT_ADAPTERINFO>);
    impl Drop for Adapters {
        fn drop(&mut self) {
            for adapter in &self.0 {
                if adapter.hAdapter != 0 {
                    unsafe {
                        D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER {
                            hAdapter: adapter.hAdapter,
                        });
                    }
                }
            }
        }
    }
    // Query directly into a bounded array: some Windows drivers reject the NULL count query.
    let mut native = Adapters(vec![D3DKMT_ADAPTERINFO::default(); 32]);
    let mut enumeration = D3DKMT_ENUMADAPTERS2 {
        NumAdapters: 32,
        pAdapters: native.0.as_mut_ptr(),
    };
    let status = unsafe { D3DKMTEnumAdapters2(&mut enumeration) };
    if status != 0
        || enumeration.NumAdapters == 0
        || enumeration.NumAdapters as usize > native.0.len()
    {
        return Err(Error::input(format!(
            "kernel GPU adapter list incomplete (status={status:#x}, count={})",
            enumeration.NumAdapters
        )));
    }
    let mut coverage = Vec::new();
    let mut software = Vec::new();
    for adapter in native.0.iter().take(enumeration.NumAdapters as usize) {
        let query_value = |kind| -> Result<u32> {
            let mut value = 0u32;
            let mut query = D3DKMT_QUERYADAPTERINFO {
                hAdapter: adapter.hAdapter,
                Type: kind,
                pPrivateDriverData: (&mut value as *mut u32).cast(),
                PrivateDriverDataSize: 4,
            };
            if unsafe { D3DKMTQueryAdapterInfo(&mut query) } != 0 {
                return Err(Error::input("GPU adapter capabilities are unknown"));
            }
            Ok(value)
        };
        let kind = query_value(KMTQAITYPE_ADAPTERTYPE)?;
        let id = format!(
            "{:08x}:{:08x}",
            adapter.AdapterLuid.HighPart as u32, adapter.AdapterLuid.LowPart
        );
        if kind & (1 << 2) != 0 {
            software.push(id);
            continue;
        } // Kernel-identified software device: retain its identity for PDH accounting.
        let version = query_value(KMTQAITYPE_DRIVERVERSION)? as i32;
        if version < 2000 || query_value(KMTQAITYPE_PHYSICALADAPTERCOUNT)? != 1 {
            return Err(Error::input(
                "unsupported WDDM or linked GPU adapter coverage",
            ));
        }
        let mut query = D3DKMT_QUERYSTATISTICS {
            Type: D3DKMT_QUERYSTATISTICS_ADAPTER,
            AdapterLuid: adapter.AdapterLuid,
            ..Default::default()
        };
        if unsafe { D3DKMTQueryStatistics(&mut query) } != 0 {
            return Err(Error::input("kernel GPU engine count is unknown"));
        }
        let nodes = unsafe { query.QueryResult.AdapterInformation.NodeCount };
        if !(1..=128).contains(&nodes) {
            return Err(Error::input("kernel GPU engine count unsupported"));
        }
        coverage.push(AdapterCoverage {
            id,
            nodes,
            wddm_version: version,
        });
    }
    coverage.sort_by(|a, b| a.id.cmp(&b.id));
    software.sort();
    if coverage.is_empty() {
        return Err(Error::input("no hardware GPU adapters could be covered"));
    }
    Ok((coverage, software))
}

struct Query(PDH_HQUERY);
impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.0);
        }
    }
}
fn readings(counter: PDH_HCOUNTER) -> Result<Vec<EngineReading>> {
    let (mut bytes, mut count) = (0u32, 0u32);
    if unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut bytes,
            &mut count,
            ptr::null_mut(),
        )
    } != PDH_MORE_DATA
        || bytes == 0
        || bytes > 4 * 1024 * 1024
    {
        return Err(Error::input("PDH GPU engine client coverage unavailable"));
    }
    // usize provides the API's native alignment for its item array and UTF-16 names.
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
    let capacity = buffer.len() * std::mem::size_of::<usize>();
    let pointer = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    if unsafe {
        PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, pointer)
    } != 0
        || bytes as usize > capacity
        || count == 0
        || count as usize > capacity / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()
    {
        return Err(Error::input(
            "PDH GPU engine list changed or was incomplete",
        ));
    }
    let start = buffer.as_ptr() as usize;
    let end = start + capacity;
    let mut engines = Vec::new();
    for item in unsafe { std::slice::from_raw_parts(pointer, count as usize) } {
        let address = item.szName as usize;
        if address < start || address >= end || address % 2 != 0 {
            return Err(Error::input("invalid PDH instance name"));
        }
        let text =
            unsafe { std::slice::from_raw_parts(item.szName, ((end - address) / 2).min(1024)) };
        let length = text
            .iter()
            .position(|c| *c == 0)
            .ok_or_else(|| Error::input("unterminated PDH instance name"))?;
        let name = String::from_utf16(&text[..length])
            .map_err(|_| Error::input("invalid PDH instance name encoding"))?;
        let valid = matches!(
            item.FmtValue.CStatus,
            PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
        );
        let percent = if valid {
            unsafe { item.FmtValue.Anonymous.doubleValue }
        } else {
            0.0
        };
        engines.push(engine_reading(&name, percent, valid)?);
    }
    Ok(engines)
}

pub(super) fn collect_idle_proof() -> Result<IdleProof> {
    let adapters_before = adapters()?;
    let mut handle = ptr::null_mut();
    if unsafe { PdhOpenQueryW(ptr::null(), 0, &mut handle) } != 0 {
        return Err(Error::input("PDH GPU query could not open"));
    }
    let query = Query(handle);
    let mut counter = ptr::null_mut();
    let path: Vec<_> = r"\GPU Engine(*)\Utilization Percentage"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    if unsafe { PdhAddEnglishCounterW(query.0, path.as_ptr(), 0, &mut counter) } != 0
        || unsafe { PdhCollectQueryData(query.0) } != 0
    {
        return Err(Error::input("PDH GPU engine counter is unavailable"));
    }
    let mut samples = Vec::new();
    for _ in 0..3 {
        let interval = Instant::now();
        std::thread::sleep(Duration::from_secs(1));
        if unsafe { PdhCollectQueryData(query.0) } != 0 {
            return Err(Error::input("PDH GPU engine sample failed"));
        }
        samples.push(IdleSample {
            interval_ms: interval.elapsed().as_millis() as u64,
            engines: readings(counter)?,
        });
    }
    let adapters_after = adapters()?;
    let finished_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::input("PDH sample timestamp unavailable"))?
        .as_secs_f64();
    Ok(IdleProof {source:IDLE_SOURCE.into(),status:"ok".into(),adapters_stable:adapters_before==adapters_after,
        adapters:adapters_before.0,software_adapters:adapters_before.1,samples,finished_unix,
        detail:"All WDDM hardware adapters and kernel-reported engines; no engine-type filter; resident models retained".into()})
}
