//! Host observation time shared by QMP and framebuffer evidence in one process.
//! This is neither guest execution time nor an acknowledgement of guest input.
use std::{sync::OnceLock, time::Instant};

pub(super) const ID: &str = "host/process-monotonic-nanoseconds@1";

pub(super) fn nanos() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    u64::try_from(ORIGIN.get_or_init(Instant::now).elapsed().as_nanos()).unwrap_or(u64::MAX)
}
