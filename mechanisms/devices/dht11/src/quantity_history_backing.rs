//! Generated conversion workspace model, shared in shape with Data's model.
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as S, StructuredInfoValue,
};
use conduit_plot::CheckedNativeType;
use core::mem::size_of;

fn nodes(ty: &StructuredInfoType) -> Option<usize> {
    let children = match ty.shape() {
        S::Leaf(_) => 0,
        S::Nominal { representation, .. } => nodes(representation)?,
        S::Collection { element, length } => nodes(element)?.checked_mul(usize::from(length))?,
        S::Sequence {
            element,
            maximum_items,
            ..
        } => nodes(element)?.checked_mul(usize::from(maximum_items))?,
        S::Record { fields, .. } => fields
            .iter()
            .try_fold(0usize, |n, f| n.checked_add(nodes(f.value_type())?))?,
        S::Variant { cases, .. } => cases
            .iter()
            .try_fold(0usize, |n, c| Some(n.max(nodes(c.payload_type())?)))?,
    };
    children.checked_add(1)
}
fn generated_encoding(native: &CheckedNativeType, maximum: usize) -> Option<usize> {
    if !native.invariants.is_empty() {
        return None;
    }
    let mut metadata = native
        .value_contracts
        .capacity()
        .checked_mul(size_of::<conduit_plot::NativeTypeValueContract>())?;
    for contract in &native.value_contracts {
        metadata = metadata
            .checked_add(contract.representation_path.capacity())?
            .checked_add(contract.contract.retained_backing_bytes()?)?;
    }
    // Data's generated conversion model: schema aliases, value/field trees,
    // bounded leaf text and canonical Vec growth plus contract metadata. Four
    // copies cover concurrently converted trees and geometric Vec capacity.
    native
        .value_type
        .retained_backing_bytes()?
        .checked_mul(4)?
        .checked_add(nodes(&native.value_type)?.checked_mul(
            4 * (size_of::<StructuredInfoValue>() + size_of::<StructuredFieldValue>()),
        )?)?
        .checked_add(maximum.checked_mul(4)?)?
        .checked_add(metadata.checked_mul(4)?)
}

/// Additional preparation workspace beyond the retained recorder/converter.
/// Includes returned native snapshot, simultaneous input vectors, clone encoding,
/// shared validation collision scratch and final caller encoding/frozen bytes.
/// Generated checked Type construction belongs to the admitted catalog/preflight
/// owner; no arbitrary-schema compiler temporary bound is claimed here.
pub(super) fn conversion_bound() -> Option<usize> {
    let batch = conduit_data::checked_quantity_observation_batch_type().ok()?;
    let snapshot = conduit_data::checked_quantity_observation_snapshot_type().ok()?;
    // Refused members can encode larger than4096 before extent refusal. Use
    // the complete schema ceiling, never the successful selected-file extent.
    let batch_bytes = conduit_data::resolved_quantity_observation_batch_byte_bound().ok()? as usize;
    let snapshot_bytes =
        conduit_data::resolved_quantity_observation_snapshot_byte_bound().ok()? as usize;
    let batch_native = conduit_data::resolved_quantity_observation_batch_backing_bound();
    let snapshot_native = conduit_data::resolved_quantity_observation_snapshot_backing_bound();
    let batch_encoding = generated_encoding(&batch, batch_bytes)?;
    let snapshot_encoding = generated_encoding(&snapshot, snapshot_bytes)?;
    // Two original channel vectors/batches + a clone being encoded; snapshot
    // bound includes the shared validator's finite subject-collision scratch.
    batch_native
        .checked_mul(3)?
        .checked_add(snapshot_native.checked_mul(2)?)?
        .checked_add(batch_encoding)?
        .checked_add(snapshot_encoding)?
        // Final encoding may coexist with returned snapshot and frozen backing.
        .checked_add(snapshot_bytes.checked_mul(2)?)
}
