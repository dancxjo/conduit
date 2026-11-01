//! Finite accepted-Fore preparation custody; no durable receipt or authority.
use super::native::{CompletedScope, LiveFore, SourceSelection};
use conduit_environment_measurement::source_observation::{ObservationContract, MAXIMUM_BYTES};
use std::sync::Arc;

pub(in super::super) const CAPACITY: usize = 16;
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Refusal {
    Scope,
    Observation,
    Sequence,
    Conflict,
    Capacity,
}
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Accepted {
    Added,
    Duplicate,
}
pub(in super::super) struct StagedObservation {
    sequence: u64,
    length: usize,
    bytes: [u8; MAXIMUM_BYTES],
}
impl StagedObservation {
    pub(in super::super) fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(in super::super) fn canonical(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
/// Prepared for one independently selected Source before its Play begins.
/// The first accepted completion freezes the actual native Play. No rotation,
/// implicit clearing, or eviction is provided: the owner must retain this value
/// across worker completion and workset replacement until explicit disposition.
pub(in super::super) struct SourceRecordingStage {
    body_id: conduit_body::BodyId,
    selection: Arc<SourceSelection>,
    scope: Option<Arc<CompletedScope>>,
    contract: ObservationContract,
    slots: [Option<StagedObservation>; CAPACITY],
    length: usize,
    prepared: Option<checkpoint::PreparedSourceChunk>,
    verified: Option<super::recording_run::VerifiedSourceSnapshot>,
}
#[path = "source_recording_stage/catalog.rs"]
pub(crate) mod catalog;
#[path = "source_recording_stage/checkpoint.rs"]
mod checkpoint;
#[path = "source_recording_stage/publication.rs"]
pub(crate) mod publication;
#[path = "source_recording_stage/recovery.rs"]
pub(crate) mod recovery;
impl SourceRecordingStage {
    pub(in super::super) fn prepare(
        body_id: conduit_body::BodyId,
        selection: Arc<SourceSelection>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            body_id,
            selection,
            scope: None,
            contract: ObservationContract::prepare().map_err(|_| Refusal::Observation)?,
            slots: std::array::from_fn(|_| None),
            length: 0,
            prepared: None,
            verified: None,
        })
    }
    pub(in super::super) fn verified_reference(&self) -> Option<&conduit_core::BoundedResourceRef> {
        let verified = self.verified.as_ref()?;
        self.prepared
            .as_ref()
            .filter(|p| &p.reference == verified.reference())
            .map(|p| &p.reference)
    }
    pub(super) fn accept_verified_snapshot(
        &mut self,
        witness: super::recording_run::VerifiedSourceSnapshot,
    ) -> Result<(), String> {
        if self
            .prepared
            .as_ref()
            .is_none_or(|p| &p.reference != witness.reference())
        {
            return Err("verified snapshot differs from retained Source prefix".into());
        }
        self.verified = Some(witness);
        Ok(())
    }
    pub(in super::super) fn len(&self) -> usize {
        self.length
    }
    pub(in super::super) fn scope(&self) -> Option<&CompletedScope> {
        self.scope.as_deref()
    }
    pub(in super::super) fn observations(&self) -> impl Iterator<Item = &StagedObservation> {
        self.slots[..self.length].iter().filter_map(Option::as_ref)
    }
    pub(in super::super) fn check_selection(
        &self,
        body_id: &conduit_body::BodyId,
        selection: &SourceSelection,
    ) -> Result<(), Refusal> {
        if body_id == &self.body_id && selection == self.selection.as_ref() {
            Ok(())
        } else {
            Err(Refusal::Scope)
        }
    }
    pub(in super::super) fn validate(&self, fore: &LiveFore) -> Result<Accepted, Refusal> {
        self.check(fore)
    }
    /// All validation precedes mutation, including first-Play scope adoption.
    pub(in super::super) fn accept(&mut self, fore: &LiveFore) -> Result<Accepted, Refusal> {
        let accepted = self.check(fore)?;
        if accepted == Accepted::Duplicate {
            return Ok(accepted);
        }
        let mut bytes = [0; MAXIMUM_BYTES];
        bytes[..fore.length].copy_from_slice(&fore.bytes[..fore.length]);
        self.slots[self.length] = Some(StagedObservation {
            sequence: fore.sequence,
            length: fore.length,
            bytes,
        });
        self.scope.get_or_insert_with(|| Arc::clone(&fore.scope));
        self.length += 1;
        Ok(Accepted::Added)
    }
    fn check(&self, fore: &LiveFore) -> Result<Accepted, Refusal> {
        if fore.scope.selection.as_ref() != self.selection.as_ref()
            || fore.scope.native_play.host_id != self.selection.host_id
            || fore.scope.native_play.boot_id != self.selection.boot_id
            || fore.scope.native_play.plan_id != self.selection.plan_id
            || fore.scope.native_play
                != conduit_core::bind_active_play(
                    &self.selection.plan_id,
                    &self.selection.host_id,
                    &self.selection.boot_id,
                    fore.scope.native_play.play_sequence,
                )
            || self
                .scope
                .as_ref()
                .is_some_and(|scope| scope.native_play != fore.scope.native_play)
        {
            return Err(Refusal::Scope);
        }
        if fore.length == 0
            || fore.length > MAXIMUM_BYTES
            || self.contract.decode(&fore.bytes[..fore.length]).is_err()
        {
            return Err(Refusal::Observation);
        }
        if let Some(old) = self
            .observations()
            .find(|old| old.sequence() == fore.sequence)
        {
            return if old.canonical() == &fore.bytes[..fore.length] {
                Ok(Accepted::Duplicate)
            } else {
                Err(Refusal::Conflict)
            };
        }
        if fore.sequence != self.length as u64 {
            return Err(Refusal::Sequence);
        }
        if self.length == CAPACITY {
            return Err(Refusal::Capacity);
        }
        Ok(Accepted::Added)
    }
}
#[cfg(test)]
#[path = "source_recording_stage/tests.rs"]
mod tests;
