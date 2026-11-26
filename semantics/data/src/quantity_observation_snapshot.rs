//! A finite view of independently resolved channels, never a recorder or read grant.
use crate::{
    checked_quantity_observation_batch_type, resolved_quantity_observation_batch_backing_bound,
    resolved_quantity_observation_batch_backing_bytes, validate_quantity_observation_batch,
    QuantityObservationInputRefusal as Refusal, ResolvedQuantityObservationBatch,
    ResolvedQuantityObservationSnapshot,
};
use alloc::{format, string::String, vec::Vec};
use conduit_core::{PreparedStructuredValueValidator, StructuredInfoType};
use conduit_plot::{rust_binding::NativeRustBinding, CheckedNativeType};
use core::mem::size_of;

pub const MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES: usize = 2;
pub const MAX_QUANTITY_OBSERVATION_SNAPSHOT_MEMBER_BYTES: usize = 4096;
pub const MAX_QUANTITY_OBSERVATION_SNAPSHOT_BYTES: usize = 16 * 1024;

pub fn checked_quantity_observation_snapshot_type() -> Result<CheckedNativeType, Refusal> {
    crate::generated::checked_quantity_observation_snapshot_type().map_err(|_| Refusal::Preparation)
}

pub fn resolved_quantity_observation_snapshot_type() -> Result<StructuredInfoType, Refusal> {
    ResolvedQuantityObservationSnapshot::semantic_type().map_err(|_| Refusal::Preparation)
}

/// Canonical Type prefix plus one record/sequence envelope and at most two
/// standalone <=4096-byte batch nodes. Nested values omit their own Type prefix.
/// Refuse preparation if a future schema cannot fit the existing worker profile.
pub fn resolved_quantity_observation_snapshot_byte_bound() -> Result<u32, Refusal> {
    let batch = checked_quantity_observation_batch_type()?;
    let batch_prefix = batch
        .value_type
        .canonical_bytes()
        .map_err(|_| Refusal::Preparation)?
        .len();
    let snapshot = resolved_quantity_observation_snapshot_type()?;
    let prefix = snapshot
        .canonical_bytes()
        .map_err(|_| Refusal::Preparation)?
        .len();
    let maximum = MAX_QUANTITY_OBSERVATION_SNAPSHOT_MEMBER_BYTES
        .checked_sub(batch_prefix)
        .and_then(|node| node.checked_mul(MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES))
        // Record tag/count, field-name length/name, sequence tag/count.
        .and_then(|nodes| nodes.checked_add(prefix + 5 + 4 + "batches".len() + 5))
        .filter(|bytes| *bytes <= MAX_QUANTITY_OBSERVATION_SNAPSHOT_BYTES)
        .ok_or(Refusal::Preparation)?;
    u32::try_from(maximum).map_err(|_| Refusal::Preparation)
}

pub struct PreparedQuantityObservationSnapshotInput {
    validator: PreparedStructuredValueValidator,
    maximum_bytes: u32,
    workspace_bytes: usize,
}
impl PreparedQuantityObservationSnapshotInput {
    pub fn prepare() -> Result<Self, Refusal> {
        let native = checked_quantity_observation_snapshot_type()?;
        if !native.invariants.is_empty() {
            return Err(Refusal::Preparation);
        }
        let maximum_bytes = resolved_quantity_observation_snapshot_byte_bound()?;
        let validator =
            PreparedStructuredValueValidator::new(&native.value_type, maximum_bytes as usize)
                .map_err(|_| Refusal::Preparation)?;
        let workspace_bytes =
            crate::quantity_observation_input::workspace_bound(&native, maximum_bytes as usize)
                .and_then(|n| n.checked_add(resolved_quantity_observation_snapshot_backing_bound()))
                .ok_or(Refusal::Preparation)?;
        Ok(Self {
            validator,
            maximum_bytes,
            workspace_bytes,
        })
    }
    pub fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.validator.retained_backing_bytes()
    }
    /// Includes generated decode trees/metadata, returned owned snapshot,
    /// per-member canonical checks and finite subject-identity collision scratch.
    /// Source input custody and the resulting Face need separate IMAGE charges.
    pub fn projection_workspace_bytes(&self) -> Option<usize> {
        Some(self.workspace_bytes)
    }
    pub fn decode(&self, canonical: &[u8]) -> Result<ResolvedQuantityObservationSnapshot, Refusal> {
        self.validator
            .validate(canonical)
            .map_err(|_| Refusal::Encoding)?;
        let snapshot = ResolvedQuantityObservationSnapshot::decode(canonical)
            .map_err(|_| Refusal::Encoding)?;
        validate_quantity_observation_snapshot(&snapshot)?;
        Ok(snapshot)
    }
}

/// Preserve the existing per-batch sequence/clock/quantity laws independently.
/// Namespaced sample identities match the existing Face projection contract:
/// `<source-identity>/sample/<sequence>`. Every emitted subject must be unique,
/// including collisions between those derived names and another root/channel.
pub fn validate_quantity_observation_snapshot(
    snapshot: &ResolvedQuantityObservationSnapshot,
) -> Result<(), Refusal> {
    if snapshot.batches.is_empty()
        || snapshot.batches.len() > MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES
    {
        return Err(Refusal::Capacity);
    }
    let mut subjects: Vec<String> = Vec::with_capacity(
        MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES * (2 + crate::MAX_RESOLVED_QUANTITY_OBSERVATIONS),
    );
    for batch in snapshot.batches.iter() {
        validate_quantity_observation_batch(batch)?;
        let encoded = batch.clone().encode().map_err(|_| Refusal::Encoding)?;
        if encoded.len() > MAX_QUANTITY_OBSERVATION_SNAPSHOT_MEMBER_BYTES {
            return Err(Refusal::Capacity);
        }
        for identity in [&batch.source_identity, &batch.channel_identity] {
            if subjects.contains(identity) {
                return Err(Refusal::Identity);
            }
            subjects.push(identity.clone());
        }
        for observation in batch.observations.iter() {
            let identity = format!("{}/sample/{}", batch.source_identity, observation.sequence);
            if subjects.contains(&identity) {
                return Err(Refusal::Identity);
            }
            subjects.push(identity);
        }
    }
    Ok(())
}

pub fn resolved_quantity_observation_snapshot_backing_bound() -> usize {
    size_of::<ResolvedQuantityObservationSnapshot>()
        + 2 * MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES
            * (size_of::<ResolvedQuantityObservationBatch>() + resolved_quantity_observation_batch_backing_bound())
        // Validation owns at most36 subject names with156B payload each;2x
        // String formatting capacity allowance. No retained history is created.
        + MAX_QUANTITY_OBSERVATION_SNAPSHOT_BATCHES * (2 + crate::MAX_RESOLVED_QUANTITY_OBSERVATIONS)
            * (size_of::<String>() + 2 * (128 + "/sample/".len() + 20))
}

pub fn resolved_quantity_observation_snapshot_backing_bytes(
    snapshot: &ResolvedQuantityObservationSnapshot,
) -> Option<usize> {
    let mut bytes = size_of::<ResolvedQuantityObservationSnapshot>().checked_add(
        snapshot
            .batches
            .physical_capacity()
            .checked_mul(size_of::<ResolvedQuantityObservationBatch>())?,
    )?;
    for batch in snapshot.batches.iter() {
        bytes = bytes.checked_add(
            resolved_quantity_observation_batch_backing_bytes(batch)?
                .checked_sub(size_of::<ResolvedQuantityObservationBatch>())?,
        )?;
    }
    Some(bytes)
}

pub fn quantity_observation_snapshot_input_preparation_backing_bound() -> Result<usize, Refusal> {
    let prepared = PreparedQuantityObservationSnapshotInput::prepare()?;
    size_of::<PreparedQuantityObservationSnapshotInput>()
        .checked_add(
            prepared
                .retained_backing_bytes()
                .ok_or(Refusal::Preparation)?,
        )
        .and_then(|n| n.checked_add(prepared.workspace_bytes))
        .ok_or(Refusal::Preparation)
}
