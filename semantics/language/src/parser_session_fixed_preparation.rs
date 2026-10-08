//! Complete fixed Source/Native/original Plan admission before any target ingress.
use crate::{
    parser_production_families::port_descriptors,
    parser_session_execution::{
        verification::PreparedSourceVerification, ParserSessionEntry,
        ParserSessionVerificationLimits, ParserSessionVerificationReceipt,
    },
    parser_session_fixed_bindings::prepare_fixed_binding,
    parser_session_fixed_ingress::{FixedRefusal, PreparedParserFixedIngress},
    parser_session_numeric_plan_storage::numeric_plan_retained_bytes,
    parser_session_source_plan::{
        validate_fixed_source_plan_seal, validate_fixed_source_plan_structure,
    },
    parser_session_source_plan_storage::{
        source_plan_preparation_reservation, SourcePlanPreparationReservation,
    },
    parser_session_target_contract::ParserSessionPreparedTarget,
    parser_source_native_parity::verify_source_native_parity,
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::PreparedNativeFamily;
use core::cell::RefCell;
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedPreparationLimits {
    pub(crate) verification: ParserSessionVerificationLimits,
    pub(crate) maximum_metadata_temporary_bytes: usize,
    pub(crate) maximum_plan_validation_temporary_bytes: usize,
    pub(crate) maximum_endpoint_encoding_requested_bytes: usize,
    pub(crate) maximum_existing_target_bytes: usize,
    /// Includes every other live Session owner and shared immutable resource.
    pub(crate) other_existing_session_reserved_bytes: usize,
    pub(crate) maximum_combined_bytes: usize,
    pub(crate) maximum_invocations: u32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedPreparationReceipt {
    pub(crate) source: ParserSessionVerificationReceipt,
    pub(crate) original_plan: SourcePlanPreparationReservation,
    pub(crate) metadata_temporary_bytes_bound: usize,
    pub(crate) combined_declared_bytes_bound: usize,
}
pub(crate) fn prepare_fixed_target<E: ParserSessionPreparedTarget>(
    executor: E,
    entry: ParserSessionEntry,
    family: Rc<RefCell<PreparedNativeFamily>>,
    limits: FixedPreparationLimits,
) -> Result<(PreparedParserFixedIngress<E>, FixedPreparationReceipt), FixedRefusal<E::Error>> {
    use FixedRefusal as R;
    let (input_descriptor, output_descriptor) = port_descriptors(entry).ok_or(R::Entry)?;
    let contract = executor.storage_contract();
    // This complete declared envelope is checked before the first Session-owned
    // allocation. Opaque existing target storage remains the target's contract.
    let combined = limits
        .other_existing_session_reserved_bytes
        .checked_add(contract.combined_bytes())
        .and_then(|n| n.checked_add(limits.verification.preparation_peak_bytes))
        .and_then(|n| n.checked_add(limits.maximum_metadata_temporary_bytes))
        .and_then(|n| n.checked_add(limits.maximum_plan_validation_temporary_bytes))
        .and_then(|n| n.checked_add(limits.maximum_endpoint_encoding_requested_bytes))
        .ok_or(R::Pressure)?;
    if combined > limits.maximum_combined_bytes
        || contract.retained_bytes() > limits.maximum_existing_target_bytes
        || limits.maximum_invocations == 0
    {
        return Err(R::Pressure);
    }
    if executor.entry() != entry.name()
        || executor.input_type_bytes() != input_descriptor.type_bytes
        || executor.output_type_bytes() != output_descriptor.type_bytes
    {
        return Err(R::Entry);
    }
    {
        let family = family.borrow();
        if !family.contains_descriptor(input_descriptor)
            || !family.contains_descriptor(output_descriptor)
        {
            return Err(R::Descriptor);
        }
    }
    let original_plan = executor.original_plan_owner();
    if original_plan.as_ref() != executor.original_plan() {
        return Err(R::Plan);
    }
    validate_fixed_source_plan_structure(executor.expanded_source(), &original_plan, entry)
        .map_err(|_| R::Plan)?;
    if numeric_plan_retained_bytes(&original_plan).map_err(|_| R::Pressure)?
        > contract.retained_bytes()
    {
        return Err(R::Pressure);
    }
    let metadata = verify_source_native_parity(
        executor.checked_source(),
        &family.borrow(),
        &[input_descriptor, output_descriptor],
        limits.maximum_metadata_temporary_bytes,
    )
    .map_err(|_| R::Descriptor)?;
    let (verifier, input, output, source) =
        PreparedSourceVerification::prepare(entry, limits.verification).map_err(|_| R::Source)?;
    let plan_reservation = source_plan_preparation_reservation(
        &original_plan,
        entry,
        source.preparation_peak_heap_bytes_bound,
        limits.maximum_plan_validation_temporary_bytes,
    )
    .map_err(|_| R::Pressure)?;
    validate_fixed_source_plan_seal(executor.expanded_source(), &original_plan, entry)
        .map_err(|_| R::Plan)?;
    let ingress = prepare_fixed_binding(
        executor,
        entry,
        original_plan,
        family,
        verifier,
        &input,
        &output,
        limits.maximum_endpoint_encoding_requested_bytes,
        contract,
        limits.maximum_invocations,
    )?;
    Ok((
        ingress,
        FixedPreparationReceipt {
            source,
            original_plan: plan_reservation,
            metadata_temporary_bytes_bound: metadata,
            combined_declared_bytes_bound: combined,
        },
    ))
}
