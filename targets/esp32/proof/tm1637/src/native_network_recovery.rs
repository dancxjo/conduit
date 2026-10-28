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
    // AP services are acknowledged and owners returned; new input is accepted
    // while the same saved revision waits for its next finite attempt burst.
    Cooldown { at: u64 },
    Joining { deadline: u64 },
    Station,
    Restoring,
    Faulted,
}
/// Checked preparation eligibility, never selected ownership or radio authority.
/// Private fields prevent a caller manufacturing the next generation.
pub struct Candidate {
    revision: u64,
    generation: u64,
    at: u64,
}
impl Candidate {
    pub fn generation(&self) -> u64 {
        self.generation
    }
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
        if !self.access_point_only() {
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
            Phase::Waiting { at } | Phase::Cooldown { at } if now >= at && workspace => {
                let candidate = self.preview_attempt(now)?.ok_or(Refusal::StaleAttempt)?;
                self.begin_attempt(candidate, now)
            }
            Phase::Joining { deadline } if now >= deadline => self.restore(),
            Phase::Station if !station_link_address_services => self.restore(),
            _ => Ok(Action::None),
        }
    }
    /// Preview is immutable: allocation, planning or provider preparation may
    /// refuse without spending a generation or starting a cleanup deadline.
    pub fn preview_attempt(&self, now: u64) -> Result<Option<Candidate>, Refusal> {
        if now < self.last_now {
            return Err(Refusal::Clock);
        }
        if !self.attempt_due(now) {
            return Ok(None);
        }
        let generation = self.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
        now.checked_add(15_000).ok_or(Refusal::Clock)?;
        Ok(Some(Candidate {
            revision: self.revision,
            generation,
            at: now,
        }))
    }
    /// Cold callers invoke this only after preparation, immediately before the
    /// selected installer can issue/retain affine possession. Installer refusal
    /// remains terminal custody; this entrance never resets an attempted Play.
    pub fn begin_attempt(&mut self, candidate: Candidate, now: u64) -> Result<Action, Refusal> {
        if now < candidate.at {
            return Err(Refusal::Clock);
        }
        let current = self.preview_attempt(now)?.ok_or(Refusal::StaleAttempt)?;
        if candidate.revision != current.revision || candidate.generation != current.generation {
            return Err(Refusal::StaleAttempt);
        }
        let deadline = now.checked_add(15_000).ok_or(Refusal::Clock)?;
        self.clock(now)?;
        self.generation = candidate.generation;
        if matches!(self.phase, Phase::Cooldown { .. }) {
            self.attempts = 0;
        }
        self.attempts += 1;
        self.phase = Phase::Joining { deadline };
        Ok(Action::PrepareAttempt {
            revision: candidate.revision,
            generation: candidate.generation,
        })
    }
    pub fn attached(&mut self, generation: u64) -> Result<(), Refusal> {
        self.require_generation(generation)?;
        if !matches!(self.phase, Phase::Joining { .. }) {
            return Err(Refusal::StaleAttempt);
        }
        self.attempts = 0;
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
            Phase::Cooldown {
                at: now.checked_add(60_000).ok_or(Refusal::Clock)?,
            }
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
        matches!(self.phase, Phase::Waiting { .. } | Phase::Cooldown { .. })
    }
    pub fn access_point_only(&self) -> bool {
        matches!(self.phase, Phase::AccessPoint | Phase::Cooldown { .. })
    }
    /// Eligibility only: callers still acquire actual preparation workspace and
    /// returned owners. Monitoring must poll with workspace=false.
    pub fn attempt_due(&self, now: u64) -> bool {
        now >= self.last_now
            && matches!(self.phase, Phase::Waiting { at } | Phase::Cooldown { at } if now >= at)
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
        assert_eq!(r.poll(66_102, true, false), Ok(Action::None));
        assert!(r.attempt_due(66_103));
        assert_eq!(r.poll(100_000, false, false), Ok(Action::None));
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
    fn repeated_cooldowns_preserve_generations_and_accept_new_configuration() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        let mut now = 0;
        for generation in 1..=300 {
            assert!(r.attempt_due(now));
            assert_eq!(r.poll(now, false, false), Ok(Action::None));
            assert_eq!(
                r.poll(now, true, false),
                Ok(Action::PrepareAttempt {
                    revision: 1,
                    generation,
                })
            );
            assert_eq!(r.attached(generation + 1), Err(Refusal::StaleAttempt));
            assert_eq!(
                r.failed(generation),
                Ok(Action::RestoreAccessPoint { generation })
            );
            assert_eq!(
                r.restored(generation + 1, now, true),
                Err(Refusal::StaleAttempt)
            );
            r.restored(generation, now, true).unwrap();
            let delay = match generation % 3 {
                1 => 1_000,
                2 => 5_000,
                _ => 60_000,
            };
            assert_eq!(r.access_point_only(), generation % 3 == 0);
            assert!(!r.attempt_due(now + delay - 1));
            assert_eq!(r.poll(now + delay - 1, true, false), Ok(Action::None));
            now += delay;
            // A monitoring poll at the deadline cannot spend or reset a burst.
            assert_eq!(r.poll(now, false, false), Ok(Action::None));
            assert!(r.attempt_due(now));
        }
        assert_eq!(r.revision(), 1);
        assert_eq!(r.generation(), 300);
        assert_eq!(r.configuration(1, now), Err(Refusal::StaleConfiguration));
        r.configuration(2, now).unwrap();
        assert_eq!(
            r.poll(now, true, false),
            Ok(Action::PrepareAttempt {
                revision: 2,
                generation: 301,
            })
        );
    }
    #[test]
    fn preview_refusal_does_not_spend_generation_or_start_deadline() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        let first = r.preview_attempt(0).unwrap().unwrap();
        assert_eq!(first.generation(), 1);
        // Cold provider/planner refusal discards only this candidate. No radio
        // owner, revision, generation or attempt is reset to manufacture retry.
        let _ = first;
        assert_eq!(r.poll(60_000, false, false), Ok(Action::None));
        assert_eq!(r.generation(), 0);
        assert_eq!(r.attempts, 0);
        let candidate = r.preview_attempt(60_000).unwrap().unwrap();
        let duplicate = r.preview_attempt(60_000).unwrap().unwrap();
        assert_eq!(
            r.begin_attempt(candidate, 61_000),
            Ok(Action::PrepareAttempt {
                revision: 1,
                generation: 1,
            })
        );
        assert_eq!(
            r.begin_attempt(duplicate, 61_000),
            Err(Refusal::StaleAttempt)
        );
        assert_eq!(r.poll(75_999, false, false), Ok(Action::None));
        assert_eq!(
            r.poll(76_000, false, false),
            Ok(Action::RestoreAccessPoint { generation: 1 })
        );
    }
    #[test]
    fn candidate_cannot_commit_after_configuration_change_or_clock_regression() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        let candidate = r.preview_attempt(10).unwrap().unwrap();
        assert_eq!(r.begin_attempt(candidate, 9), Err(Refusal::Clock));
        assert_eq!(r.generation(), 0);
        let candidate = r.preview_attempt(10).unwrap().unwrap();
        // A restored AP cooldown accepts a new revision; the old eligibility
        // value can never become authority for that different configuration.
        r.phase = Phase::Cooldown { at: 10 };
        r.configuration(2, 10).unwrap();
        assert_eq!(r.begin_attempt(candidate, 10), Err(Refusal::StaleAttempt));
        assert_eq!(r.generation(), 0);
        let candidate = r.preview_attempt(10).unwrap().unwrap();
        assert_eq!(
            r.begin_attempt(candidate, 10),
            Ok(Action::PrepareAttempt {
                revision: 2,
                generation: 1,
            })
        );
    }
    #[test]
    fn cooldown_clock_and_generation_exhaustion_never_release_an_attempt() {
        let mut r = Recovery::new();
        r.revision = 1;
        r.phase = Phase::Restoring;
        r.generation = 3;
        r.attempts = 3;
        assert_eq!(r.restored(3, u64::MAX - 59_999, true), Err(Refusal::Clock));
        assert!(!r.attempt_due(u64::MAX));
        assert_eq!(r.poll(u64::MAX, true, false), Ok(Action::None));
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        r.generation = u64::MAX;
        assert_eq!(r.poll(0, true, false), Err(Refusal::Exhausted));
        assert_eq!(r.generation(), u64::MAX);
        assert_eq!(r.attempts, 0);
        assert_eq!(r.attached(u64::MAX), Err(Refusal::StaleAttempt));
    }
    #[test]
    fn successful_attachment_starts_fresh_outage_burst_and_cleanup_failure_stops() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        r.poll(0, true, false).unwrap();
        r.failed(1).unwrap();
        r.restored(1, 0, true).unwrap();
        r.poll(1_000, true, false).unwrap();
        r.failed(2).unwrap();
        r.restored(2, 1_000, true).unwrap();
        r.poll(6_000, true, false).unwrap();
        r.attached(3).unwrap();
        assert_eq!(
            r.poll(7_000, false, false),
            Ok(Action::RestoreAccessPoint { generation: 3 })
        );
        r.restored(3, 7_000, true).unwrap();
        assert!(!r.access_point_only());
        assert!(!r.attempt_due(11_999));
        assert!(r.attempt_due(12_000));
        r.poll(12_000, true, false).unwrap();
        r.failed(4).unwrap();
        r.restored(4, 12_000, false).unwrap();
        assert!(!r.attempt_due(u64::MAX));
        assert_eq!(r.poll(u64::MAX, true, false), Ok(Action::None));
        assert_eq!(r.configuration(2, u64::MAX), Err(Refusal::Busy));
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
