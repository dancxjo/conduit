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
        if !self.accepts_configuration() {
            return Err(Refusal::Busy);
        }
        self.clock(now)?;
        self.revision = revision;
        self.attempts = 0;
        self.phase = Phase::Waiting { at: now };
        Ok(())
    }
    /// Acknowledged ingress may request retirement of a current station owner.
    /// This queues only the existing cleanup action: the caller retains ingress,
    /// the current revision and all native ownership until real restoration and
    /// retirement finish. Joining cannot be superseded while custody is in flight.
    pub fn configuration_retirement(&mut self, now: u64) -> Result<Action, Refusal> {
        if now < self.last_now {
            return Err(Refusal::Clock);
        }
        match self.phase {
            Phase::Station => {
                self.clock(now)?;
                self.restore()
            }
            Phase::Joining { .. } | Phase::Faulted => Err(Refusal::Busy),
            _ => Ok(Action::None),
        }
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
    /// Root-only confirmation after the exact selected scan's mandatory
    /// evidence committed and the actual AP owner returned. A scan spends the
    /// next physical generation without inventing/refunding a join attempt.
    /// `renewed` records the separate known-profile decision, never authorizes it.
    pub fn scan_retired(
        &mut self,
        generation: u64,
        now: u64,
        renewed: bool,
    ) -> Result<(), Refusal> {
        self.check_scan_retirement(generation, now)?;
        let at = now
            .checked_add(if renewed { 1_000 } else { 30_000 })
            .ok_or(Refusal::Clock)?;
        let phase = match self.phase {
            Phase::Waiting { at: old } => Phase::Waiting { at: old.max(at) },
            Phase::Cooldown { at: old } => Phase::Cooldown { at: old.max(at) },
            _ => return Err(Refusal::Busy),
        };
        self.clock(now)?;
        self.generation = generation;
        self.phase = phase;
        Ok(())
    }
    /// Immutable preflight before returned-owner/evidence settlement. It spends
    /// neither a generation nor a retry; scan_retired repeats these checks.
    pub fn check_scan_retirement(&self, generation: u64, now: u64) -> Result<(), Refusal> {
        if now < self.last_now {
            return Err(Refusal::Clock);
        }
        let next = self.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
        if generation != next || generation == u64::MAX {
            return Err(Refusal::StaleAttempt);
        }
        now.checked_add(30_000).ok_or(Refusal::Clock)?;
        if !matches!(self.phase, Phase::Waiting { .. } | Phase::Cooldown { .. }) {
            return Err(Refusal::Busy);
        }
        Ok(())
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Eligibility only: actual provider cleanup and retirement remain required.
    /// Inspecting this phase neither acknowledges restoration nor spends work.
    pub fn is_restoring(&self) -> bool {
        self.phase == Phase::Restoring
    }
    pub fn is_joining(&self) -> bool {
        matches!(self.phase, Phase::Joining { .. })
    }
    pub fn awaiting_attempt(&self) -> bool {
        matches!(self.phase, Phase::Waiting { .. } | Phase::Cooldown { .. })
    }
    /// Configuration intent may supersede an unspent retry only after callers
    /// prove actual returned native custody; this predicate grants no owner.
    pub fn accepts_configuration(&self) -> bool {
        matches!(
            self.phase,
            Phase::AccessPoint | Phase::Waiting { .. } | Phase::Cooldown { .. }
        )
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
    fn scan_retirement_preflight_neither_spends_generation_nor_moves_clock() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        let candidate = r.preview_attempt(0).unwrap().unwrap();
        r.begin_attempt(candidate, 0).unwrap();
        r.failed(1).unwrap();
        r.restored(1, 1, true).unwrap();
        let original = (r.phase, r.revision, r.generation, r.attempts, r.last_now);
        assert_eq!(r.check_scan_retirement(2, 1001), Ok(()));
        assert_eq!(r.check_scan_retirement(2, 1001), Ok(()));
        assert_eq!(r.check_scan_retirement(3, 1001), Err(Refusal::StaleAttempt));
        assert_eq!(r.check_scan_retirement(2, 0), Err(Refusal::Clock));
        assert_eq!(r.check_scan_retirement(2, u64::MAX), Err(Refusal::Clock));
        assert_eq!(
            (r.phase, r.revision, r.generation, r.attempts, r.last_now),
            original
        );
        // No physical scan occurred: the original join candidate is unchanged.
        assert_eq!(r.preview_attempt(1001).unwrap().unwrap().generation(), 2);
    }
    #[test]
    fn retired_scan_advances_shared_generation_without_refunding_join_attempts() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        let candidate = r.preview_attempt(0).unwrap().unwrap();
        r.begin_attempt(candidate, 0).unwrap();
        r.failed(1).unwrap();
        r.restored(1, 1, true).unwrap();
        r.scan_retired(2, 1001, false).unwrap();
        assert_eq!((r.generation, r.attempts, r.revision), (2, 1, 1));
        assert!(r.preview_attempt(31_000).unwrap().is_none());
        let candidate = r.preview_attempt(31_001).unwrap().unwrap();
        assert_eq!(candidate.generation(), 3);
        r.begin_attempt(candidate, 31_001).unwrap();
        assert_eq!((r.generation, r.attempts), (3, 2));
        let old = (r.phase, r.generation, r.attempts, r.last_now);
        assert_eq!(r.scan_retired(4, 31_002, true), Err(Refusal::Busy));
        assert_eq!((r.phase, r.generation, r.attempts, r.last_now), old);
    }
    #[test]
    fn scan_cooldown_keeps_exhausted_burst_until_existing_attempt_boundary() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        for (generation, now) in [(1, 0), (2, 1000), (3, 6000)] {
            let candidate = r.preview_attempt(now).unwrap().unwrap();
            r.begin_attempt(candidate, now).unwrap();
            r.failed(generation).unwrap();
            r.restored(generation, now, true).unwrap();
        }
        assert_eq!(r.phase, Phase::Cooldown { at: 66_000 });
        r.scan_retired(4, 66_000, true).unwrap();
        assert_eq!((r.generation, r.attempts), (4, 3));
        assert!(r.preview_attempt(66_999).unwrap().is_none());
        let candidate = r.preview_attempt(67_000).unwrap().unwrap();
        r.begin_attempt(candidate, 67_000).unwrap();
        assert_eq!((r.generation, r.attempts), (5, 1));
    }
    #[test]
    fn scan_retirement_refusals_preserve_policy_and_never_wrap() {
        let mut r = Recovery::new();
        r.configuration(1, 100).unwrap();
        for (generation, now, expected) in [
            (0, 100, Refusal::StaleAttempt),
            (2, 100, Refusal::StaleAttempt),
            (1, 99, Refusal::Clock),
            (1, u64::MAX, Refusal::Clock),
        ] {
            let old = (r.phase, r.generation, r.attempts, r.last_now);
            assert_eq!(r.scan_retired(generation, now, true), Err(expected));
            assert_eq!((r.phase, r.generation, r.attempts, r.last_now), old);
        }
        r.generation = u64::MAX;
        assert_eq!(r.scan_retired(1, 100, true), Err(Refusal::Exhausted));
        assert_eq!(r.generation, u64::MAX);
    }
    #[test]
    fn pending_retirement_storage_can_retry_without_new_attempt_or_acknowledgment() {
        let mut recovery = Recovery::new();
        recovery.configuration(1, 0).unwrap();
        let candidate = recovery.preview_attempt(0).unwrap().unwrap();
        recovery.begin_attempt(candidate, 0).unwrap();
        assert!(!recovery.is_restoring());
        assert_eq!(
            recovery.failed(1),
            Ok(Action::RestoreAccessPoint { generation: 1 })
        );
        let attempts = recovery.attempts;
        for now in [1, 2, 15_000, 30_000] {
            assert!(recovery.is_restoring());
            assert_eq!(recovery.poll(now, true, false), Ok(Action::None));
            assert!(recovery.preview_attempt(now).unwrap().is_none());
            assert_eq!(recovery.attempts, attempts);
            assert_eq!(recovery.generation(), 1);
            assert_eq!(recovery.revision(), 1);
        }
        recovery.restored(1, 30_000, true).unwrap();
        assert!(!recovery.is_restoring());
        assert_eq!(recovery.attempts, attempts);
        assert!(recovery.preview_attempt(30_000).unwrap().is_none());
        assert_eq!(
            recovery
                .preview_attempt(31_000)
                .unwrap()
                .unwrap()
                .generation(),
            2
        );
    }
    #[test]
    fn new_configuration_supersedes_unspent_waiting_without_spending_attempt() {
        let mut r = Recovery::new();
        r.configuration(1, 0).unwrap();
        r.poll(0, true, false).unwrap();
        assert_eq!(r.configuration(2, 1), Err(Refusal::Busy));
        r.failed(1).unwrap();
        assert_eq!(r.configuration(2, 2), Err(Refusal::Busy));
        r.restored(1, 3, true).unwrap();
        assert!(!r.access_point_only());
        assert!(r.accepts_configuration());
        let stale = r.preview_attempt(1003).unwrap().unwrap();
        // Repeated constructor refusals never begin this old candidate.
        assert_eq!(r.generation(), 1);
        r.configuration(2, 1003).unwrap();
        assert_eq!(r.revision(), 2);
        assert_eq!(r.generation(), 1);
        assert_eq!(r.begin_attempt(stale, 1003), Err(Refusal::StaleAttempt));
        let current = r.preview_attempt(1003).unwrap().unwrap();
        assert_eq!(
            r.begin_attempt(current, 1003),
            Ok(Action::PrepareAttempt {
                revision: 2,
                generation: 2
            })
        );
    }
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

#[cfg(test)]
#[path = "native_network_recovery/configuration_tests.rs"]
mod configuration_tests;
