//! One journey origin shared by QMP, framebuffer and raw serial observations.
//! Host observations are distinct from guest consumption and physical scanout.
use std::time::Instant;

pub(super) const ID: &str = "std::time::Instant";

pub(super) fn nanos(origin: Instant) -> u64 {
    u64::try_from(origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
