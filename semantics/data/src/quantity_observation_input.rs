//! Exact finite reported observations. Decoding grants no producer or storage authority.
//! Owned decoding allocates in separately funded hosted preparation, never a Step.
use crate::{QuantityObservationOutcome, ResolvedQuantityObservationBatch};
use conduit_core::{
    PreparedStructuredValueValidator, Quantity, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape as Shape, StructuredInfoValue,
};
use conduit_plot::{rust_binding::NativeRustBinding, CheckedNativeType};
use core::mem::size_of;

pub const MAX_RESOLVED_QUANTITY_OBSERVATIONS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityObservationInputRefusal {
    Preparation,
    Encoding,
    Capacity,
    Identity,
    Order,
    Gap,
    Quantity,
}

/// Complete owner metadata, including every primitive refinement. Cross-event
/// order/gap/family laws are additionally checked by the prepared decoder below.
pub fn checked_quantity_observation_batch_type(
) -> Result<CheckedNativeType, QuantityObservationInputRefusal> {
    crate::generated::checked_quantity_observation_batch_type()
        .map_err(|_| QuantityObservationInputRefusal::Preparation)
}

pub fn resolved_quantity_observation_batch_type(
) -> Result<StructuredInfoType, QuantityObservationInputRefusal> {
    ResolvedQuantityObservationBatch::semantic_type()
        .map_err(|_| QuantityObservationInputRefusal::Preparation)
}

/// Conservative canonical ceiling from the exact closed schema. All Text leaves
/// in this Type are <=256 bytes; per-field generated contracts enforce tighter
/// limits. Schema prefix is included once, leaf framing conservatively twice.
pub fn resolved_quantity_observation_batch_byte_bound(
) -> Result<u32, QuantityObservationInputRefusal> {
    let ty = resolved_quantity_observation_batch_type()?;
    let bytes = ty
        .canonical_bytes()
        .ok()
        .and_then(|prefix| prefix.len().checked_add(node_bytes(&ty)?))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .filter(|bytes| *bytes as usize <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .ok_or(QuantityObservationInputRefusal::Preparation)?;
    Ok(bytes)
}

pub struct PreparedQuantityObservationInput {
    validator: PreparedStructuredValueValidator,
    maximum_bytes: u32,
    workspace_bytes: usize,
}
impl PreparedQuantityObservationInput {
    pub fn prepare() -> Result<Self, QuantityObservationInputRefusal> {
        let native = checked_quantity_observation_batch_type()?;
        // This closed family has primitive contracts and explicit cross-event
        // checks, not allocating portable expression laws. Refuse future laws
        // until their evaluator/metadata storage gets an owning budget.
        if !native.invariants.is_empty() {
            return Err(QuantityObservationInputRefusal::Preparation);
        }
        let maximum_bytes = resolved_quantity_observation_batch_byte_bound()?;
        let validator =
            PreparedStructuredValueValidator::new(&native.value_type, maximum_bytes as usize)
                .map_err(|_| QuantityObservationInputRefusal::Preparation)?;
        let workspace_bytes = workspace_bound(&native, maximum_bytes as usize)
            .ok_or(QuantityObservationInputRefusal::Preparation)?;
        Ok(Self {
            validator,
            maximum_bytes,
            workspace_bytes,
        })
    }

    pub fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }

    /// Heap capacities retained by this prepared decoder, excluding inline Self,
    /// allocator overhead and stack. The original IMAGE must charge these.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.validator.retained_backing_bytes()
    }

    /// Conservative transient heap payload during generated owned decoding,
    /// including the returned batch. Input custody and output Face/escaped
    /// contribution are independent charges. No allocation-free claim is made.
    pub fn projection_workspace_bytes(&self) -> Option<usize> {
        Some(self.workspace_bytes)
    }

    pub fn decode(
        &self,
        canonical: &[u8],
    ) -> Result<ResolvedQuantityObservationBatch, QuantityObservationInputRefusal> {
        self.validator
            .validate(canonical)
            .map_err(|_| QuantityObservationInputRefusal::Encoding)?;
        let batch = ResolvedQuantityObservationBatch::decode(canonical)
            .map_err(|_| QuantityObservationInputRefusal::Encoding)?;
        validate_quantity_observation_batch(&batch)?;
        Ok(batch)
    }
}

/// Validate native values independently of serialization. Native fields are
/// public for composition, so consumers must not treat construction as admission.
pub fn validate_quantity_observation_batch(
    batch: &ResolvedQuantityObservationBatch,
) -> Result<(), QuantityObservationInputRefusal> {
    use QuantityObservationInputRefusal as R;
    for (text, maximum) in [
        (&batch.source_identity, 128),
        (&batch.source_name, 128),
        (&batch.channel_identity, 128),
        (&batch.channel_name, 128),
        (&batch.producer_host, 256),
        (&batch.producer_boot, 256),
        (&batch.producer_plan, 256),
        (&batch.producer_play, 256),
    ] {
        if text.is_empty() || text.len() > maximum {
            return Err(R::Identity);
        }
    }
    if batch.source_identity == batch.channel_identity {
        return Err(R::Identity);
    }
    if batch.observations.len() > MAX_RESOLVED_QUANTITY_OBSERVATIONS {
        return Err(R::Capacity);
    }
    let mut prior: Option<(u64, u64)> = None;
    let mut quantity: Option<Quantity> = None;
    for sample in batch.observations.iter() {
        if sample.sequence == 0 {
            return Err(R::Order);
        }
        if let Some(gap) = &sample.gap_before {
            if gap.first_sequence == 0
                || gap.first_sequence > gap.last_sequence
                || gap.last_sequence.checked_add(1) != Some(sample.sequence)
            {
                return Err(R::Gap);
            }
        }
        if let Some((sequence, time)) = prior {
            if sample.sequence <= sequence || sample.sampled_at_boot_ms < time {
                return Err(R::Order);
            }
            let expected =
                (sample.sequence - sequence > 1).then_some((sequence + 1, sample.sequence - 1));
            let gap = sample
                .gap_before
                .as_ref()
                .map(|gap| (gap.first_sequence, gap.last_sequence));
            if gap != expected {
                return Err(R::Gap);
            }
        }
        match &sample.outcome {
            QuantityObservationOutcome::Available(value) => {
                if quantity.is_some_and(|previous| {
                    previous.family() != value.family() || previous.role() != value.role()
                }) {
                    return Err(R::Quantity);
                }
                quantity = Some(**value);
            }
            QuantityObservationOutcome::Unavailable(reason) => {
                if reason.is_empty() || reason.len() > 128 {
                    return Err(R::Encoding);
                }
            }
        }
        prior = Some((sample.sequence, sample.sampled_at_boot_ms));
    }
    Ok(())
}

fn node_bytes(ty: &StructuredInfoType) -> Option<usize> {
    Some(match ty.shape() {
        Shape::Leaf(kind) if kind.as_str() == "value/text" => 5 + 256,
        Shape::Leaf(_) => {
            5 + conduit_plot::maximum_prepared_canonical_value_bytes(ty).ok()? as usize
        }
        Shape::Nominal { representation, .. } => node_bytes(representation)?,
        Shape::Collection { element, length } => {
            5 + node_bytes(element)?.checked_mul(usize::from(length))?
        }
        Shape::Sequence {
            element,
            maximum_items,
            ..
        } => 5 + node_bytes(element)?.checked_mul(usize::from(maximum_items))?,
        Shape::Record { fields, .. } => fields.iter().try_fold(5usize, |n, field| {
            n.checked_add(4 + field.name().len())?
                .checked_add(node_bytes(field.value_type())?)
        })?,
        Shape::Variant { cases, .. } => {
            1 + cases.iter().try_fold(0usize, |n, case| {
                Some(n.max(4 + case.tag().len() + node_bytes(case.payload_type())?))
            })?
        }
    })
}

fn value_nodes(ty: &StructuredInfoType) -> Option<usize> {
    let children = match ty.shape() {
        Shape::Leaf(_) => 0,
        Shape::Nominal { representation, .. } => value_nodes(representation)?,
        Shape::Collection { element, length } => {
            value_nodes(element)?.checked_mul(usize::from(length))?
        }
        Shape::Sequence {
            element,
            maximum_items,
            ..
        } => value_nodes(element)?.checked_mul(usize::from(maximum_items))?,
        Shape::Record { fields, .. } => fields.iter().try_fold(0usize, |n, field| {
            n.checked_add(value_nodes(field.value_type())?)
        })?,
        Shape::Variant { cases, .. } => cases.iter().try_fold(0usize, |n, case| {
            Some(n.max(value_nodes(case.payload_type())?))
        })?,
    };
    children.checked_add(1)
}

pub(crate) fn workspace_bound(native: &CheckedNativeType, maximum: usize) -> Option<usize> {
    // Same exact-schema-first model as the ambient projection owner: two copies
    // of schema/value trees and input bytes, plus generated metadata and owned
    // native materialization. Fourfold vector/text allowance covers growth and
    // simultaneous validation/conversion temporaries; no expression evaluators.
    let nodes = value_nodes(&native.value_type)?;
    let schema = native.value_type.retained_backing_bytes()?;
    let mut contracts = native
        .value_contracts
        .capacity()
        .checked_mul(size_of::<conduit_plot::NativeTypeValueContract>())?;
    for contract in &native.value_contracts {
        contracts = contracts
            .checked_add(contract.representation_path.capacity())?
            .checked_add(contract.contract.retained_backing_bytes()?)?;
    }
    schema
        .checked_mul(4)?
        .checked_add(nodes.checked_mul(
            4 * (size_of::<StructuredInfoValue>() + size_of::<StructuredFieldValue>()),
        )?)?
        .checked_add(maximum.checked_mul(4)?)?
        .checked_add(contracts.checked_mul(4)?)?
        .checked_add(resolved_quantity_observation_batch_backing_bound())
}

/// Returned owned batch, including inline Self; excludes allocator overhead.
/// Strings are materialized from bounded leaves; a twofold capacity allowance
/// covers their construction and sequence collection's growth policy.
pub fn resolved_quantity_observation_batch_backing_bound() -> usize {
    size_of::<ResolvedQuantityObservationBatch>()
        + 2 * (4 * 128 + 4 * 256)
        + 2 * MAX_RESOLVED_QUANTITY_OBSERVATIONS
            * (size_of::<crate::ResolvedQuantityObservation>() + size_of::<Quantity>() + 128)
}

pub fn resolved_quantity_observation_batch_backing_bytes(
    batch: &ResolvedQuantityObservationBatch,
) -> Option<usize> {
    let mut bytes = size_of::<ResolvedQuantityObservationBatch>().checked_add(
        batch
            .observations
            .physical_capacity()
            .checked_mul(size_of::<crate::ResolvedQuantityObservation>())?,
    )?;
    for text in [
        &batch.source_identity,
        &batch.source_name,
        &batch.channel_identity,
        &batch.channel_name,
        &batch.producer_host,
        &batch.producer_boot,
        &batch.producer_plan,
        &batch.producer_play,
    ] {
        bytes = bytes.checked_add(text.capacity())?;
    }
    for sample in batch.observations.iter() {
        bytes = bytes.checked_add(match &sample.outcome {
            QuantityObservationOutcome::Available(_) => size_of::<Quantity>(),
            QuantityObservationOutcome::Unavailable(reason) => reason.capacity(),
        })?;
    }
    Some(bytes)
}

/// Decoder plus its bounded transient owned decode workspace. Caller adds its
/// projection output, original input custody and escaped contribution charges.
pub fn quantity_observation_input_preparation_backing_bound(
) -> Result<usize, QuantityObservationInputRefusal> {
    let prepared = PreparedQuantityObservationInput::prepare()?;
    size_of::<PreparedQuantityObservationInput>()
        .checked_add(
            prepared
                .retained_backing_bytes()
                .ok_or(QuantityObservationInputRefusal::Preparation)?,
        )
        .and_then(|n| n.checked_add(prepared.workspace_bytes))
        .ok_or(QuantityObservationInputRefusal::Preparation)
}
