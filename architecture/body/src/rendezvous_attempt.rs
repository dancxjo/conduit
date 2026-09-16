//! Finite attempt scheduling and exact outcome evidence for spawn rendezvous.

use alloc::{string::String, vec::Vec};

use crate::{
    RendezvousCandidate, RendezvousDescriptorRefusal, SpawnRendezvousDescriptor,
    MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE, MAX_RENDEZVOUS_CANDIDATES,
};

pub const MAX_RENDEZVOUS_ATTEMPT_RECORDS: usize =
    MAX_RENDEZVOUS_CANDIDATES * MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE as usize;

/// One explicit Line attempt selected from a reviewed descriptor.
///
/// This grants transport reachability only. It grants no membership authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendezvousAttempt<'a> {
    pub candidate: &'a RendezvousCandidate,
    pub attempt: u8,
    pub timeout_millis: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousAttemptDecision<'a> {
    Try(RendezvousAttempt<'a>),
    Exhausted,
}

/// Deterministic finite candidate order for one self-joining start.
pub struct RendezvousAttemptSchedule<'a> {
    descriptor: &'a SpawnRendezvousDescriptor,
    candidate_index: usize,
    attempts_on_candidate: u8,
}

impl<'a> RendezvousAttemptSchedule<'a> {
    pub fn new(
        descriptor: &'a SpawnRendezvousDescriptor,
        now_millis: u64,
    ) -> Result<Self, RendezvousDescriptorRefusal> {
        descriptor.validate(now_millis)?;
        Ok(Self {
            descriptor,
            candidate_index: 0,
            attempts_on_candidate: 0,
        })
    }

    pub fn next(&mut self, now_millis: u64) -> RendezvousAttemptDecision<'a> {
        while let Some(candidate) = self.descriptor.candidates.get(self.candidate_index) {
            if candidate.expires_at_millis <= now_millis
                || self.attempts_on_candidate >= candidate.maximum_attempts
            {
                self.candidate_index += 1;
                self.attempts_on_candidate = 0;
                continue;
            }
            self.attempts_on_candidate += 1;
            return RendezvousAttemptDecision::Try(RendezvousAttempt {
                candidate,
                attempt: self.attempts_on_candidate,
                timeout_millis: candidate.attempt_timeout_millis,
            });
        }
        RendezvousAttemptDecision::Exhausted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousAttemptOutcome {
    RouteUnavailable,
    TimedOut,
    AuthenticationRefused,
    Redirected,
    TransportLost,
    Connected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RendezvousAttemptRecord {
    pub candidate_id: String,
    pub attempt: u8,
    pub timeout_millis: u32,
    pub outcome: RendezvousAttemptOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousAttemptEvidenceRefusal {
    AttemptAlreadyPending,
    NoPendingAttempt,
    AttemptMismatch,
    RecordBound,
    AlreadyConnected,
}

/// Bounded evidence journal paired with the explicit attempt schedule.
///
/// A connected Line is still not admission. The invitation proof and Body
/// admission decision remain separate later evidence.
#[derive(Default)]
pub struct RendezvousAttemptJournal {
    pending: Option<(String, u8, u32)>,
    records: Vec<RendezvousAttemptRecord>,
    connected: bool,
}

impl RendezvousAttemptJournal {
    pub fn begin(
        &mut self,
        attempt: RendezvousAttempt<'_>,
    ) -> Result<(), RendezvousAttemptEvidenceRefusal> {
        if self.connected {
            return Err(RendezvousAttemptEvidenceRefusal::AlreadyConnected);
        }
        if self.pending.is_some() {
            return Err(RendezvousAttemptEvidenceRefusal::AttemptAlreadyPending);
        }
        if self.records.len() >= MAX_RENDEZVOUS_ATTEMPT_RECORDS {
            return Err(RendezvousAttemptEvidenceRefusal::RecordBound);
        }
        self.pending = Some((
            attempt.candidate.candidate_id.clone(),
            attempt.attempt,
            attempt.timeout_millis,
        ));
        Ok(())
    }

    pub fn finish(
        &mut self,
        candidate_id: &str,
        attempt: u8,
        outcome: RendezvousAttemptOutcome,
    ) -> Result<(), RendezvousAttemptEvidenceRefusal> {
        let Some((expected_candidate, expected_attempt, timeout_millis)) = self.pending.take()
        else {
            return Err(RendezvousAttemptEvidenceRefusal::NoPendingAttempt);
        };
        if candidate_id != expected_candidate || attempt != expected_attempt {
            self.pending = Some((expected_candidate, expected_attempt, timeout_millis));
            return Err(RendezvousAttemptEvidenceRefusal::AttemptMismatch);
        }
        self.records.push(RendezvousAttemptRecord {
            candidate_id: expected_candidate,
            attempt,
            timeout_millis,
            outcome,
        });
        self.connected = outcome == RendezvousAttemptOutcome::Connected;
        Ok(())
    }

    pub fn records(&self) -> &[RendezvousAttemptRecord] {
        &self.records
    }

    pub const fn connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RendezvousAuthentication, RendezvousLineFamily, RENDEZVOUS_DESCRIPTOR_PROTOCOL};
    use alloc::vec;

    fn candidate(id: &str) -> RendezvousCandidate {
        RendezvousCandidate {
            candidate_id: id.into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: "tls://body.example:443/conduit".into(),
            authentication: RendezvousAuthentication {
                server_identity: "body/key-1".into(),
                transport_binding_sha256: [1; 32],
            },
            expires_at_millis: 10_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
        }
    }

    #[test]
    fn exact_failure_evidence_precedes_finite_candidate_fallback() {
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![
                candidate("candidate/primary"),
                candidate("candidate/fallback"),
            ],
        };
        let mut schedule = RendezvousAttemptSchedule::new(&descriptor, 1_000).unwrap();
        let mut journal = RendezvousAttemptJournal::default();

        let RendezvousAttemptDecision::Try(primary) = schedule.next(1_000) else {
            panic!("primary attempt missing");
        };
        journal.begin(primary).unwrap();
        journal
            .finish(
                "candidate/primary",
                1,
                RendezvousAttemptOutcome::AuthenticationRefused,
            )
            .unwrap();
        let RendezvousAttemptDecision::Try(fallback) = schedule.next(1_000) else {
            panic!("fallback attempt missing");
        };
        journal.begin(fallback).unwrap();
        journal
            .finish("candidate/fallback", 1, RendezvousAttemptOutcome::Connected)
            .unwrap();

        assert_eq!(journal.records().len(), 2);
        assert!(journal.connected());
        assert_eq!(
            journal.records()[0].outcome,
            RendezvousAttemptOutcome::AuthenticationRefused
        );
    }

    #[test]
    fn outcome_cannot_be_attached_to_a_different_attempt() {
        let descriptor = SpawnRendezvousDescriptor {
            protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
            body_id: "body/one".into(),
            invitation_id: "invitation/one".into(),
            candidates: vec![candidate("candidate/only")],
        };
        let mut schedule = RendezvousAttemptSchedule::new(&descriptor, 1_000).unwrap();
        let RendezvousAttemptDecision::Try(attempt) = schedule.next(1_000) else {
            panic!("attempt missing");
        };
        let mut journal = RendezvousAttemptJournal::default();
        journal.begin(attempt).unwrap();
        assert_eq!(
            journal.finish(
                "candidate/substitute",
                1,
                RendezvousAttemptOutcome::Connected
            ),
            Err(RendezvousAttemptEvidenceRefusal::AttemptMismatch)
        );
        assert!(!journal.connected());
        assert!(journal.records().is_empty());
    }
}
