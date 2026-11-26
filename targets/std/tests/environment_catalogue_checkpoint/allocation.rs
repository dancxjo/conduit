//! Actual calling-thread allocations between the public installed Play markers.
//! Preparation, other threads, allocator RSS and confinement are separate claims.
#[allow(dead_code)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
use conduit_std_host::{KernelTimingPhase, TimerAdapter};
use std::time::Duration;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
#[derive(Default)]
pub struct PlayProbe {
    interval: Option<probe::ObservationInterval>,
    observation: Option<probe::Observation>,
}
impl TimerAdapter for PlayProbe {
    fn wait(&mut self, _: Duration) {}
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        match phase {
            KernelTimingPhase::PreparationStarted => {}
            KernelTimingPhase::PlayStarted => {
                assert!(self.interval.is_none() && self.observation.is_none());
                self.interval = Some(probe::ObservationInterval::begin());
            }
            KernelTimingPhase::PlayFinished => {
                self.observation = Some(self.interval.take().expect("Play started").finish());
            }
        }
    }
}
impl PlayProbe {
    pub fn assert_finished_without_allocation(&self) {
        let measured = self.observation.expect("both actual Play markers");
        assert_eq!(measured.allocation_attempts, 0);
        assert_eq!(measured.allocations, 0);
        assert_eq!(measured.reallocations, 0);
        assert_eq!(measured.requested_bytes, 0);
    }
}
