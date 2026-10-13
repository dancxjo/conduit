use crate::{
    body_causal_evidence::BodyEventTimeObservation, body_execution::ObservedKernelEvent,
    TimerAdapter,
};
use conduit_core::{BootId, HostId};
use conduit_kernel::KernelEvent;

pub struct KernelClockObservations {
    seen: usize,
    last: Option<conduit_core::MonotonicInstant>,
    supported: bool,
    observations: Vec<ObservedKernelEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{MonotonicClockIdentity, MonotonicInstant, TemporalScale};
    use conduit_kernel::{KernelEventKind, NodeId};
    use std::time::Duration;

    struct SampleTimer {
        samples: Vec<Option<MonotonicInstant>>,
    }

    impl TimerAdapter for SampleTimer {
        fn wait(&mut self, _duration: Duration) {}

        fn monotonic_observation(
            &mut self,
            _host_id: &HostId,
            _boot_id: &BootId,
        ) -> Option<MonotonicInstant> {
            self.samples.remove(0)
        }
    }

    fn sample(ticks: u64, boot_id: &str) -> Option<MonotonicInstant> {
        let identity = MonotonicClockIdentity::new(
            "host/test".into(),
            boot_id.into(),
            "test/provider".into(),
            TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap();
        Some(MonotonicInstant::new(ticks, identity).unwrap())
    }

    fn event(sequence: u32) -> KernelEvent {
        KernelEvent {
            sequence,
            node: NodeId(0),
            port: None,
            request: None,
            kind: KernelEventKind::BackCompleted,
        }
    }

    #[test]
    fn captures_each_new_event_once_and_refuses_changed_or_regressed_basis() {
        let mut observations = KernelClockObservations::with_capacity(4);
        let mut timer = SampleTimer {
            samples: vec![sample(10, "boot/test"), sample(9, "boot/test")],
        };
        let host_id = HostId::from("host/test");
        let boot_id = BootId::from("boot/test");
        observations.capture_new([event(1)].into_iter(), &mut timer, &host_id, &boot_id);
        observations.capture_new(
            [event(1), event(2)].into_iter(),
            &mut timer,
            &host_id,
            &boot_id,
        );
        observations.capture_new(
            [event(1), event(2), event(3)].into_iter(),
            &mut timer,
            &host_id,
            &boot_id,
        );
        let retained = observations.into_observations();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].sequence, 1);
        assert_eq!(retained[0].time.local().ticks(), 10);
    }

    #[test]
    fn wrong_boot_never_enters_host_observations() {
        let mut observations = KernelClockObservations::with_capacity(1);
        let mut timer = SampleTimer {
            samples: vec![sample(10, "boot/other")],
        };
        observations.capture_new(
            [event(1)].into_iter(),
            &mut timer,
            &HostId::from("host/test"),
            &BootId::from("boot/test"),
        );
        assert!(observations.into_observations().is_empty());
    }
}

impl KernelClockObservations {
    pub fn with_capacity(event_capacity: usize) -> Self {
        Self {
            seen: 0,
            last: None,
            supported: true,
            observations: Vec::with_capacity(event_capacity),
        }
    }

    pub fn capture_new<T: TimerAdapter>(
        &mut self,
        events: impl Iterator<Item = KernelEvent>,
        timer: &mut T,
        host_id: &HostId,
        boot_id: &BootId,
    ) {
        for event in events.skip(self.seen) {
            self.seen += 1;
            if !self.supported {
                continue;
            }
            let Some(local) = timer.monotonic_observation(host_id, boot_id) else {
                self.supported = false;
                continue;
            };
            if local.clock().host_id() != host_id
                || local.clock().boot_id() != boot_id
                || self.last.as_ref().is_some_and(|last| {
                    last.clock() != local.clock() || local.ticks() < last.ticks()
                })
            {
                self.supported = false;
                continue;
            }
            let Ok(time) = BodyEventTimeObservation::new(local, None) else {
                self.supported = false;
                continue;
            };
            self.last = Some(time.local().clone());
            if self.observations.len() < self.observations.capacity() {
                self.observations.push(ObservedKernelEvent {
                    sequence: event.sequence,
                    time,
                });
            }
        }
    }

    pub fn into_observations(self) -> Vec<ObservedKernelEvent> {
        self.observations
    }
}
