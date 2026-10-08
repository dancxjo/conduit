//! Reviewed immutable six-profile no-cache preparation and separately derived
//! per-Back construction bounds. Never a generic Type/checker reservation.
use super::*;
use crate::fixed_numeric_finite_envelope::FiniteEnvelope;
pub const IMMUTABLE_FLOW_PAIR_KINDS: [&str; 6] = [
    "numeric/flow-pair40x164",
    "numeric/flow-pair40-164x160",
    "numeric/flow-pair40-164-160x128",
    "numeric/flow-pair40-164-160-128x128",
    "numeric/flow-pair40-164-160-128-128x256",
    "numeric/flow-pair40-164-160-128-128-256x1",
];
// Reviewed immutable no-cache profile only. Exact selected SDK/toolchain/source
// artifacts and exhaustive all-six allocator gates are part of the proof basis.
pub const FLOW_PAIR_PROFILE_PREPARATION_REQUESTED_BYTES: usize = 64 * 1024 * 1024;
pub const FLOW_PAIR_PROFILE_PREPARATION_PEAK_BYTES: usize = 1024 * 1024;
pub const FLOW_PAIR_PROFILE_RETAINED_BYTES: usize = 32 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct FlowPairProfilePreparationLimits {
    pub maximum_preparation_requested_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_retained_heap_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlowPairStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub preparation_peak_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_accounted_heap_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowPairStorageRefusal {
    Capacity,
    ForeignKind,
    UnownedHostedCache,
    ProfileNotPrepared,
    Shape,
    Composition(PreparedStructuredCompositionStorageRefusal),
}
pub struct PreparedFixedFlowPairProfile {
    pair_type: StructuredInfoType,
    maximum: [u32; 2],
    left: FiniteEnvelope,
    right: FiniteEnvelope,
    offer: CapabilityOffer,
}
impl PreparedFixedFlowPairProfile {
    pub fn prepare_immutable_no_cache(
        identity: &str,
        limits: FlowPairProfilePreparationLimits,
    ) -> Result<(Self, FlowPairStorageReceipt), FlowPairStorageRefusal> {
        if cfg!(feature = "hosted-catalog-cache") {
            return Err(FlowPairStorageRefusal::UnownedHostedCache);
        }
        if limits.maximum_preparation_requested_bytes
            < FLOW_PAIR_PROFILE_PREPARATION_REQUESTED_BYTES
            || limits.maximum_preparation_peak_bytes < FLOW_PAIR_PROFILE_PREPARATION_PEAK_BYTES
            || limits.maximum_retained_heap_bytes < FLOW_PAIR_PROFILE_RETAINED_BYTES
        {
            return Err(FlowPairStorageRefusal::Capacity);
        }
        if !IMMUTABLE_FLOW_PAIR_KINDS.contains(&identity) {
            return Err(FlowPairStorageRefusal::ForeignKind);
        }
        let source_identity = identity.replacen("numeric/flow-pair", "numeric/pair", 1);
        let (_, pair_type, kind) = fixed_numeric_pair_contracts()
            .map_err(|_| FlowPairStorageRefusal::Shape)?
            .into_iter()
            .find(|(_, _, kind)| kind.kind_id.as_str() == source_identity)
            .ok_or(FlowPairStorageRefusal::Shape)?;
        let StructuredInfoTypeShape::Record { fields, .. } = pair_type.shape() else {
            return Err(FlowPairStorageRefusal::Shape);
        };
        if fields.len() != 2 {
            return Err(FlowPairStorageRefusal::Shape);
        }
        let left_type = fields[0].value_type();
        let right_type = fields[1].value_type();
        let maximum = [
            maximum_prepared_canonical_value_bytes(left_type)
                .map_err(|_| FlowPairStorageRefusal::Shape)?,
            maximum_prepared_canonical_value_bytes(right_type)
                .map_err(|_| FlowPairStorageRefusal::Shape)?,
        ];
        let left = FiniteEnvelope::prepare(left_type).map_err(|_| FlowPairStorageRefusal::Shape)?;
        let right =
            FiniteEnvelope::prepare(right_type).map_err(|_| FlowPairStorageRefusal::Shape)?;
        let offer = offer_for_contract(as_flow_contract(kind, identity), identity);
        let profile = Self {
            pair_type,
            maximum,
            left,
            right,
            offer,
        };
        let accounted = profile.owned_heap_bytes()?;
        if accounted > FLOW_PAIR_PROFILE_RETAINED_BYTES {
            return Err(FlowPairStorageRefusal::Capacity);
        }
        Ok((
            profile,
            FlowPairStorageReceipt {
                preparation_requested_bytes_bound: FLOW_PAIR_PROFILE_PREPARATION_REQUESTED_BYTES,
                preparation_peak_bytes_bound: FLOW_PAIR_PROFILE_PREPARATION_PEAK_BYTES,
                retained_heap_bytes_bound: FLOW_PAIR_PROFILE_RETAINED_BYTES,
                retained_accounted_heap_bytes: accounted,
            },
        ))
    }
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.pair_type
    }
    pub fn owned_heap_bytes(&self) -> Result<usize, FlowPairStorageRefusal> {
        self.pair_type
            .owned_heap_bytes()
            .checked_add(self.left.local_accounted_heap_bytes())
            .and_then(|n| n.checked_add(self.right.local_accounted_heap_bytes()))
            .and_then(|n| {
                capability_offer_owned_heap_bytes(&self.offer)
                    .ok()
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or(FlowPairStorageRefusal::Capacity)
    }
}
impl FixedFlowPairBack {
    pub fn storage_reservation(
        profile: &PreparedFixedFlowPairProfile,
    ) -> Result<FlowPairStorageReceipt, FlowPairStorageRefusal> {
        let StructuredInfoTypeShape::Record { fields, .. } = profile.pair_type.shape() else {
            return Err(FlowPairStorageRefusal::Shape);
        };
        let r = PreparedTypedTuplePairEncoder::storage_reservation(
            fields[0].value_type(),
            profile.maximum[0],
            fields[1].value_type(),
            profile.maximum[1],
        )
        .map_err(FlowPairStorageRefusal::Composition)?;
        let clones = profile
            .left
            .local_accounted_heap_bytes()
            .checked_add(profile.right.local_accounted_heap_bytes())
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        let requested = r
            .preparation_requested_bytes_bound
            .checked_add(clones)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        Ok(FlowPairStorageReceipt {
            preparation_requested_bytes_bound: requested,
            preparation_peak_bytes_bound: requested,
            retained_heap_bytes_bound: r
                .retained_heap_bytes_bound
                .checked_add(clones)
                .ok_or(FlowPairStorageRefusal::Capacity)?,
            retained_accounted_heap_bytes: 0,
        })
    }
    pub fn prepare_selected_with_storage_limits(
        profile: &PreparedFixedFlowPairProfile,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, FlowPairStorageReceipt), FlowPairStorageRefusal> {
        let mut r = Self::storage_reservation(profile)?;
        if r.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(FlowPairStorageRefusal::Capacity);
        }
        let StructuredInfoTypeShape::Record { fields, .. } = profile.pair_type.shape() else {
            return Err(FlowPairStorageRefusal::Shape);
        };
        let clones = profile
            .left
            .local_accounted_heap_bytes()
            .checked_add(profile.right.local_accounted_heap_bytes())
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        let (encoder, _) = PreparedTypedTuplePairEncoder::new_with_storage_limits(
            fields[0].value_type(),
            profile.maximum[0],
            fields[1].value_type(),
            profile.maximum[1],
            maximum_preparation_requested_bytes - clones,
            maximum_retained_heap_bytes - clones,
        )
        .map_err(FlowPairStorageRefusal::Composition)?;
        let back = Self {
            encoder,
            left: profile.left.clone(),
            right: profile.right.clone(),
            staged: false,
            committed_frames: 0,
            cancelled: false,
        };
        r.retained_accounted_heap_bytes = back.local_accounted_heap_bytes();
        Ok((back, r))
    }
}
