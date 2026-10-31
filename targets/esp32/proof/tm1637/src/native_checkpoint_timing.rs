//! Bounded scalar checkpoint diagnostics. No payload or authority is retained.
#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum Stage {
    Mutation,
    MembershipSeal,
    SignerScope,
    Prepare,
    SourceOne,
    Measure,
    FinishOne,
    Erase,
    Header,
    SourceTwo,
    Write,
    FinishTwo,
    Flush,
    Readback,
    Commit,
    Inspect,
    Carry,
    Acknowledge,
    Count,
}
pub(crate) struct Timings {
    elapsed: [u32; Stage::Count as usize],
    visited: u32,
    last: Stage,
    stamp: u64,
    started: u64,
    enabled: bool,
}
fn now() -> u64 {
    #[cfg(all(target_os = "none", feature = "target-mask-native-check"))]
    return embassy_time::Instant::now().as_micros();
    #[cfg(not(all(target_os = "none", feature = "target-mask-native-check")))]
    0
}
impl Timings {
    pub(crate) fn new(enabled: bool) -> Self {
        let stamp = if enabled { now() } else { 0 };
        Self {
            elapsed: [0; Stage::Count as usize],
            visited: 0,
            last: Stage::Mutation,
            stamp,
            started: stamp,
            enabled,
        }
    }
    pub(crate) fn enter(&mut self, stage: Stage) {
        if self.enabled {
            self.record(now());
            self.last = stage;
            self.visited |= 1 << stage as usize;
        }
    }
    fn record(&mut self, stamp: u64) {
        if self.visited != 0 {
            let delta = stamp.saturating_sub(self.stamp).min(u64::from(u32::MAX)) as u32;
            let slot = &mut self.elapsed[self.last as usize];
            *slot = slot.saturating_add(delta);
        }
        self.stamp = stamp;
    }
    pub(crate) fn report(mut self, success: bool) {
        if self.enabled {
            self.record(now());
            #[cfg(all(target_os = "none", feature = "target-mask-native-check"))]
            esp_println::println!("{{\"event\":\"body-checkpoint-timing\",\"success\":{},\"total_us\":{},\"visited\":{},\"last\":{},\"phase_us\":{:?}}}", success, self.stamp.saturating_sub(self.started), self.visited, self.last as usize, self.elapsed);
        }
        let _ = (success, self.started);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timing_is_bounded_saturating_and_disables_without_clock_work() {
        let mut t = Timings::new(false);
        t.enter(Stage::Write);
        assert_eq!(t.visited, 0);
        let mut t = Timings::new(true);
        t.enter(Stage::Measure);
        t.record(42);
        assert_eq!(t.elapsed[Stage::Measure as usize], 42);
        t.record(u64::MAX);
        assert_eq!(t.elapsed[Stage::Measure as usize], u32::MAX);
        assert!(core::mem::size_of::<Timings>() <= 112);
    }
}
