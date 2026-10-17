use conduit_core::{MonotonicDeadline, MonotonicInstant, MonotonicTimeRefusal, TemporalScale};
use conduit_time::CivilDeadlineAdmission;
use std::time::Duration;

use crate::TimerAdapter;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CivilDeadlineWaitRefusal {
    NoClock,
    Clock(MonotonicTimeRefusal),
    EarlyWake,
}

pub fn wait_admitted_civil_deadline<T: TimerAdapter>(
    timer: &mut T,
    admission: &CivilDeadlineAdmission,
) -> Result<MonotonicInstant, CivilDeadlineWaitRefusal> {
    wait_exact_deadline(timer, admission.deadline())
}

fn wait_exact_deadline<T: TimerAdapter>(
    timer: &mut T,
    deadline: &MonotonicDeadline,
) -> Result<MonotonicInstant, CivilDeadlineWaitRefusal> {
    let clock = deadline.instant().clock();
    let mut previous = None;
    for attempt in 0..=8 {
        let now = timer
            .monotonic_observation(clock.host_id(), clock.boot_id())
            .ok_or(CivilDeadlineWaitRefusal::NoClock)?;
        if previous.is_some_and(|ticks| now.ticks() < ticks) {
            return Err(CivilDeadlineWaitRefusal::Clock(
                MonotonicTimeRefusal::Regressed,
            ));
        }
        previous = Some(now.ticks());
        let remaining = deadline
            .remaining_at(&now)
            .map_err(CivilDeadlineWaitRefusal::Clock)?;
        let Some(remaining) = remaining else {
            return Ok(now);
        };
        if attempt == 8 {
            return Err(CivilDeadlineWaitRefusal::EarlyWake);
        }
        let duration = match remaining.scale() {
            TemporalScale::Seconds => Duration::from_secs(remaining.ticks()),
            TemporalScale::Milliseconds => Duration::from_millis(remaining.ticks()),
            TemporalScale::Microseconds => Duration::from_micros(remaining.ticks()),
            TemporalScale::Nanoseconds => Duration::from_nanos(remaining.ticks()),
        };
        timer.wait(duration);
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, HostId, MonotonicClockIdentity, MonotonicDuration};
    use conduit_time::{
        CivilResolutionChoice, ClockCorrelation, LocalDate, LocalDateTime, LocalTime,
        NamedTimeZone, OccurrenceInstant, TemporalInstant, TemporalScale as CivilScale,
        ZonedResolution,
    };

    struct Clock {
        current: MonotonicInstant,
        wall_ticks: u64,
        forward: bool,
        waits: usize,
    }

    impl TimerAdapter for Clock {
        fn wait(&mut self, duration: Duration) {
            self.waits += 1;
            self.current = self
                .current
                .deadline_after(MonotonicDuration::new(
                    u64::try_from(duration.as_millis()).unwrap(),
                    TemporalScale::Milliseconds,
                ))
                .unwrap()
                .instant()
                .clone();
            self.wall_ticks = if self.forward {
                self.wall_ticks + 100_000
            } else {
                self.wall_ticks.saturating_sub(100_000)
            };
        }

        fn monotonic_observation(
            &mut self,
            _host_id: &HostId,
            _boot_id: &BootId,
        ) -> Option<MonotonicInstant> {
            Some(self.current.clone())
        }
    }

    fn sample(boot: &str, ticks: u64) -> MonotonicInstant {
        MonotonicInstant::new(
            ticks,
            MonotonicClockIdentity::new(
                "host/civil".into(),
                boot.into(),
                "steady/civil".into(),
                TemporalScale::Milliseconds,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn admitted_fold() -> CivilDeadlineAdmission {
        let wall = |ticks| conduit_core::TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "time/unix-utc@1".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        };
        let local = LocalDateTime::new(
            LocalDate::new(2026, 11, 1).unwrap(),
            LocalTime::new(1, 30, 0, 0).unwrap(),
        )
        .unwrap();
        let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap();
        let occurrence = OccurrenceInstant::civil(
            TemporalInstant::new(
                "time/unix-utc@1".into(),
                1,
                CivilScale::Milliseconds,
                3_000,
                0,
            )
            .unwrap(),
            local.clone(),
            CivilResolutionChoice::FoldLater,
            zone.clone(),
        )
        .unwrap();
        let resolution = ZonedResolution::Ambiguous {
            local: conduit_core::LocalDateTime::try_from(local).unwrap(),
            zone: conduit_core::NamedTimeZone::try_from(zone).unwrap(),
            earlier: wall(2_000),
            later: wall(3_000),
        };
        let correlation = ClockCorrelation::new(
            "correlation/civil".into(),
            sample("boot/one", 100),
            wall(1_000),
            0,
        )
        .unwrap();
        CivilDeadlineAdmission::admit(occurrence, resolution, correlation, 0, 1, 10_000).unwrap()
    }

    #[test]
    fn resolved_fold_runs_on_one_monotonic_deadline_despite_wall_step() {
        let admission = admitted_fold();
        assert_eq!(admission.deadline().instant().ticks(), 2_101);
        for forward in [false, true] {
            let mut timer = Clock {
                current: sample("boot/one", 100),
                wall_ticks: 1_000,
                forward,
                waits: 0,
            };
            let fired = wait_admitted_civil_deadline(&mut timer, &admission).unwrap();
            assert_eq!(fired.ticks(), 2_101);
            assert_eq!(timer.waits, 1);
            assert_ne!(timer.wall_ticks, 1_000);
        }
    }

    #[test]
    fn wall_steps_do_not_change_an_admitted_monotonic_wait() {
        let deadline = sample("boot/one", 100)
            .deadline_after(MonotonicDuration::new(5_000, TemporalScale::Milliseconds))
            .unwrap();
        for forward in [false, true] {
            let mut timer = Clock {
                current: sample("boot/one", 100),
                wall_ticks: 10_000,
                forward,
                waits: 0,
            };
            let fired = wait_exact_deadline(&mut timer, &deadline).unwrap();
            assert_eq!(fired.ticks(), 5_100);
            assert_eq!(timer.waits, 1);
            assert_ne!(timer.wall_ticks, 10_000);
        }
    }

    #[test]
    fn rebooted_clock_cannot_satisfy_the_old_deadline() {
        let deadline = sample("boot/one", 100)
            .deadline_after(MonotonicDuration::new(5_000, TemporalScale::Milliseconds))
            .unwrap();
        let mut timer = Clock {
            current: sample("boot/two", 5_100),
            wall_ticks: 10_000,
            forward: true,
            waits: 0,
        };
        assert_eq!(
            wait_exact_deadline(&mut timer, &deadline),
            Err(CivilDeadlineWaitRefusal::Clock(
                MonotonicTimeRefusal::DifferentClock
            ))
        );
        assert_eq!(timer.waits, 0);
    }
}
