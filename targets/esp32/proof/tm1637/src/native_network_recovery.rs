//! Bounded Root recovery decisions, never radio/credential/Play authority.
//! Every Attempt requires fresh selected ownership; Restore requires actual
//! acknowledged AP services before another attempt can become eligible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    PrepareAttempt { revision: u64, generation: u64 },
    RestoreAccessPoint { generation: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    StaleConfiguration,
    StaleAttempt,
    Exhausted,
    Clock,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    AccessPoint,
    Waiting { at: u64 },
    Joining { deadline: u64 },
    Station,
    Restoring,
    Faulted,
}
pub struct Recovery {
    phase: Phase,
    revision: u64,
    generation: u64,
    attempts: u8,
    last_now: u64,
}
impl Default for Recovery {
    fn default() -> Self {
        Self::new()
    }
}
impl Recovery {
    pub const fn new() -> Self {
        Self {
            phase: Phase::AccessPoint,
            revision: 0,
            generation: 0,
            attempts: 0,
            last_now: 0,
        }
    }
    /// Called only for acknowledged ingress or a valid committed private journal.
    /// This queues intent; neither path manufactures current native possession.
    pub fn configuration(&mut self, revision: u64, now: u64) -> Result<(), Refusal> {
        if revision == 0 || revision <= self.revision {
            return Err(Refusal::StaleConfiguration);
        }
        if self.phase != Phase::AccessPoint {
            return Err(Refusal::Busy);
        }
        self.clock(now)?;
        self.revision = revision;
        self.attempts = 0;
        self.phase = Phase::Waiting { at: now };
        Ok(())
    }
    /// Workspace is an actual independent admission; native sampler custody
    /// cannot be bypassed by a timer, a credential or this policy value.
    pub fn poll(
        &mut self,
        now: u64,
        workspace: bool,
        station_link_address_services: bool,
    ) -> Result<Action, Refusal> {
        self.clock(now)?;
        match self.phase {
            Phase::Waiting { at } if now >= at && workspace => {
                let generation = self.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
                let deadline = now.checked_add(15_000).ok_or(Refusal::Clock)?;
                self.generation = generation;
                self.attempts += 1;
                self.phase = Phase::Joining { deadline };
                Ok(Action::PrepareAttempt {
                    revision: self.revision,
                    generation,
                })
            }
            Phase::Joining { deadline } if now >= deadline => self.restore(),
            Phase::Station if !station_link_address_services => self.restore(),
            _ => Ok(Action::None),
        }
    }
    pub fn attached(&mut self, generation: u64) -> Result<(), Refusal> {
        self.require_generation(generation)?;
        if !matches!(self.phase, Phase::Joining { .. }) {
            return Err(Refusal::StaleAttempt);
        }
        self.phase = Phase::Station;
        Ok(())
    }
    pub fn failed(&mut self, generation: u64) -> Result<Action, Refusal> {
        self.require_generation(generation)?;
        if !matches!(self.phase, Phase::Joining { .. }) {
            return Err(Refusal::StaleAttempt);
        }
        self.restore()
    }
    /// The actual retained owner completed its one bounded cleanup and was
    /// retired. A timeout or dropped future must report failure, never readiness.
    pub fn restored(
        &mut self,
        generation: u64,
        now: u64,
        acknowledged: bool,
    ) -> Result<(), Refusal> {
        self.require_generation(generation)?;
        if self.phase != Phase::Restoring {
            return Err(Refusal::StaleAttempt);
        }
        self.clock(now)?;
        self.phase = if !acknowledged {
            Phase::Faulted
        } else if self.attempts >= 3 {
            Phase::AccessPoint
        } else {
            Phase::Waiting {
                at: now
                    .checked_add(if self.attempts == 1 { 1_000 } else { 5_000 })
                    .ok_or(Refusal::Clock)?,
            }
        };
        Ok(())
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn is_joining(&self) -> bool {
        matches!(self.phase, Phase::Joining { .. })
    }
    pub fn awaiting_attempt(&self) -> bool {
        matches!(self.phase, Phase::Waiting { .. })
    }
    pub fn access_point_only(&self) -> bool {
        self.phase == Phase::AccessPoint
    }
    fn require_generation(&self, generation: u64) -> Result<(), Refusal> {
        if generation == 0 || generation != self.generation {
            Err(Refusal::StaleAttempt)
        } else {
            Ok(())
        }
    }
    fn restore(&mut self) -> Result<Action, Refusal> {
        self.phase = Phase::Restoring;
        Ok(Action::RestoreAccessPoint {
            generation: self.generation,
        })
    }
    fn clock(&mut self, now: u64) -> Result<(), Refusal> {
        if now < self.last_now {
            return Err(Refusal::Clock);
        }
        self.last_now = now;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_acknowledged_cleanup_and_fresh_generations_bound_retries() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        assert_eq!(r.poll(100, false, false), Ok(Action::None));
        for (generation, now, ready) in [(1, 100, 1100), (2, 1101, 6101), (3, 6102, 6102)] {
            assert_eq!(
                r.poll(now, true, false),
                Ok(Action::PrepareAttempt {
                    revision: 1,
                    generation
                })
            );
            assert_eq!(
                r.failed(generation),
                Ok(Action::RestoreAccessPoint { generation })
            );
            assert_eq!(r.poll(now + 1, true, false), Ok(Action::None));
            r.restored(generation, now + 1, true).unwrap();
            if generation != 3 {
                assert_eq!(r.poll(ready, true, false), Ok(Action::None));
            }
        }
        assert!(r.access_point_only());
        assert_eq!(r.poll(100_000, true, false), Ok(Action::None));
        assert_eq!(
            r.configuration(1, 100_000),
            Err(Refusal::StaleConfiguration)
        );
        r.configuration(2, 100_000).unwrap();
        assert_eq!(
            r.poll(100_000, true, false),
            Ok(Action::PrepareAttempt {
                revision: 2,
                generation: 4
            })
        );
    }
    #[test]
    fn link_loss_has_one_cleanup_and_no_implicit_success_or_clock_reset() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        r.poll(0, true, false).unwrap();
        assert_eq!(r.attached(2), Err(Refusal::StaleAttempt));
        r.attached(1).unwrap();
        assert_eq!(r.poll(1, true, true), Ok(Action::None));
        assert_eq!(
            r.poll(2, true, false),
            Ok(Action::RestoreAccessPoint { generation: 1 })
        );
        assert_eq!(r.poll(3, true, false), Ok(Action::None));
        assert_eq!(r.poll(1, true, false), Err(Refusal::Clock));
        r.restored(1, 4, false).unwrap();
        assert_eq!(r.poll(50_000, true, false), Ok(Action::None));
        assert_eq!(r.configuration(2, 50_000), Err(Refusal::Busy));
    }
    #[test]
    fn association_deadline_spends_attempt_and_does_not_restart() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        r.poll(0, true, false).unwrap();
        assert_eq!(r.poll(14_999, true, false), Ok(Action::None));
        assert_eq!(
            r.poll(15_000, true, false),
            Ok(Action::RestoreAccessPoint { generation: 1 })
        );
        assert_eq!(r.attached(1), Err(Refusal::StaleAttempt));
    }
}
