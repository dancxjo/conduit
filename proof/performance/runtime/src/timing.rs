//! Wall and process CPU time, with raw samples and an explicit timer control.
use crate::allocation;
use std::time::Instant;
#[derive(Debug, Clone, serde::Serialize)]
pub struct Metrics {
    pub wall_ns: u64,
    pub process_cpu_ns: Option<u64>,
    pub allocator: allocation::Counts,
}
pub fn ns(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).expect("finite measurement shorter than 584 years")
}
fn cpu() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let mut value = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut value) } != 0 {
            return None;
        }
        u64::try_from(value.tv_sec)
            .ok()?
            .checked_mul(1_000_000_000)?
            .checked_add(u64::try_from(value.tv_nsec).ok()?)
    }
    #[cfg(not(target_os = "linux"))]
    None
}
pub fn measure<T>(operation: impl FnOnce() -> T) -> (T, Metrics) {
    allocation::start();
    let cpu_start = cpu();
    let wall = Instant::now();
    let value = operation();
    let wall_ns = ns(wall.elapsed());
    let process_cpu_ns = cpu_start
        .zip(cpu())
        .and_then(|(start, end)| end.checked_sub(start));
    let allocator = allocation::finish();
    (
        value,
        Metrics {
            wall_ns,
            process_cpu_ns,
            allocator,
        },
    )
}
#[derive(Debug, serde::Serialize)]
pub struct Distribution {
    pub count: usize,
    pub min_ns: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub max_ns: u64,
}
pub fn distribution(samples: &[u64]) -> Distribution {
    assert!(!samples.is_empty());
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let percentile = |percent: usize| ordered[(ordered.len() * percent).div_ceil(100) - 1];
    Distribution {
        count: samples.len(),
        min_ns: ordered[0],
        p50_ns: percentile(50),
        p95_ns: percentile(95),
        p99_ns: percentile(99),
        max_ns: *ordered.last().unwrap(),
    }
}
pub fn timer_control(samples: u32) -> Vec<u64> {
    (0..samples)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(());
            ns(start.elapsed())
        })
        .collect()
}
