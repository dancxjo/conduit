//! Complete original Source/model preparation before ingress.
//! The enclosing Session additionally reserves all histories and revision policy.
use crate::{
    parser_model_selection::PreparedParserModelSelection,
    parser_session_canonical_ingress::{
        ParserCanonicalIngressLimits, ParserCanonicalSourceExecutor,
        PreparedCanonicalParserSessionPort,
    },
    parser_session_execution::{
        verification::PreparedSourceVerification,
        ParserSessionVerificationLimits,
    },
    parser_session_fixed_ingress::ParserSessionExecutor,
    parser_session_mixed_custody::PreparedParserMixedCustody,
    parser_session_numeric_custody::{ParserNumericExecutor, PreparedParserNumericCustody},
    parser_session_numeric_plan_storage::{
        numeric_plan_preparation_reservation, numeric_plan_retained_bytes,
    },
    parser_session_source_plan::{
        validate_fixed_source_plan_seal, validate_fixed_source_plan_structure,
    },
    parser_session_source_plan_storage::source_plan_preparation_reservation,
    parser_session_target_contract::{
        ParserSessionPreparedTarget, ParserSessionTargetStorageContract,
    },
    parser_source_native_parity::verify_source_native_parity,
};
use alloc::{rc::Rc, sync::Arc};
use conduit_ai::integer_categorical_step::{
    CategoricalCanonicalAdmissionLimits, PreparedCategoricalCanonicalAdmission,
};
use conduit_core::Plan;
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::cell::RefCell;

#[derive(Debug)]
pub(crate) enum MixedPreparationRefusal {
    Pressure,
    Plan,
    Descriptor,
    Source,
    Model,
}
#[derive(Debug)]
pub(crate) enum BoundedTargetRefusal<E> {
    Plan,
    Pressure,
    Target(E),
}

/// Original full Source owner with checked target framing and permanent stop.
/// Opaque target heap/scratch remains its explicit declared storage contract.
pub(crate) struct OwnedSourceTarget<E: ParserSessionPreparedTarget> {
    target: E,
    original: Rc<Plan>,
    contract: ParserSessionTargetStorageContract,
}
impl<E: ParserSessionPreparedTarget> Drop for OwnedSourceTarget<E> {
    fn drop(&mut self) {
        self.target.cancel();
    }
}
impl<E: ParserSessionPreparedTarget> ParserCanonicalSourceExecutor for OwnedSourceTarget<E> {
    type Error = BoundedTargetRefusal<E::Error>;
    fn cancel(&mut self) {
        self.target.cancel();
    }
    fn entry(&self) -> &str {
        self.target.entry()
    }
    fn input_type_bytes(&self) -> &[u8] {
        self.target.input_type_bytes()
    }
    fn output_type_bytes(&self) -> &[u8] {
        self.target.output_type_bytes()
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        if self.target.original_plan() != self.original.as_ref() {
            return Err(BoundedTargetRefusal::Plan);
        }
        if input.len() > self.contract.maximum_input_bytes() {
            return Err(BoundedTargetRefusal::Pressure);
        }
        let maximum = output.len().min(self.contract.maximum_output_bytes());
        let length = self
            .target
            .transact(ordinal, input, &mut output[..maximum])
            .map_err(BoundedTargetRefusal::Target)?;
        if length > maximum {
            return Err(BoundedTargetRefusal::Pressure);
        }
        Ok(length)
    }
}
pub(crate) struct OwnedNumericTarget<E: ParserNumericExecutor> {
    target: E,
    contract: ParserSessionTargetStorageContract,
}
impl<E: ParserNumericExecutor> Drop for OwnedNumericTarget<E> {
    fn drop(&mut self) {
        self.target.cancel();
    }
}
impl<E: ParserNumericExecutor> ParserNumericExecutor for OwnedNumericTarget<E> {
    type Error = BoundedTargetRefusal<E::Error>;
    fn plan(&self) -> &Plan {
        self.target.plan()
    }
    fn cancel(&mut self) {
        self.target.cancel();
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        if input.len() > self.contract.maximum_input_bytes() {
            return Err(BoundedTargetRefusal::Pressure);
        }
        let maximum = output.len().min(self.contract.maximum_output_bytes());
        let length = self
            .target
            .transact(ordinal, input, &mut output[..maximum])
            .map_err(BoundedTargetRefusal::Target)?;
        if length > maximum {
            return Err(BoundedTargetRefusal::Pressure);
        }
        Ok(length)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MixedPreparationLimits {
    pub(crate) verification: ParserSessionVerificationLimits,
    pub(crate) canonical: CategoricalCanonicalAdmissionLimits,
    pub(crate) maximum_metadata_temporary_bytes: usize,
    pub(crate) maximum_plan_validation_temporary_bytes: usize,
    pub(crate) maximum_endpoint_encoding_requested_bytes: usize,
    pub(crate) other_existing_session_reserved_bytes: usize,
    pub(crate) maximum_combined_bytes: usize,
    pub(crate) maximum_invocations: u32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct MixedPreparationReceipt {
    pub(crate) combined_declared_bytes_bound: usize,
    pub(crate) retained_source_bytes_bound: usize,
    pub(crate) retained_canonical_codec_bytes_bound: usize,
    pub(crate) concurrently_live_native_bytes_bound: usize,
    pub(crate) target_declared_retained_bytes: usize,
    pub(crate) target_declared_execution_temporary_bytes: usize,
}

/// Both targets are already constructed caller input. All their complete owned
/// metadata is retained and declared separately from Session-owned preparation.
pub(crate) fn prepare_mixed_targets<S, N>(
    source: S, numeric: N, selection: Arc<PreparedParserModelSelection>,
    family: Rc<RefCell<PreparedNativeFamily>>, limits: MixedPreparationLimits,
) -> Result<(PreparedParserMixedCustody<OwnedSourceTarget<S>, OwnedNumericTarget<N>>, MixedPreparationReceipt), MixedPreparationRefusal>
where S: ParserSessionPreparedTarget,
N: ParserSessionPreparedTarget + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    prepare_mixed_targets_for::<crate::parser_session_numeric_profile::PinnedFourSlotNumericProfile, _, _>(source, numeric, selection, family, limits, None)
}

pub(crate) fn prepare_mixed_targets_for<P, S, N>(
    source: S,
    numeric: N,
    selection: Arc<P::Selection>,
    family: Rc<RefCell<PreparedNativeFamily>>,
    limits: MixedPreparationLimits,
    feature_guard: Option<crate::parser_session_feature_guard::PreparedParserFeatureGuard>,
) -> Result<
    (
        PreparedParserMixedCustody<OwnedSourceTarget<S>, OwnedNumericTarget<N>, P>,
        MixedPreparationReceipt,
    ),
    MixedPreparationRefusal,
>
where
    P: crate::parser_session_numeric_profile::FixedParserNumericProfile,
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget
        + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    use MixedPreparationRefusal as R;
    fn add(a: usize, b: usize) -> Result<usize, R> {
        a.checked_add(b).ok_or(R::Pressure)
    }
    // Guards own cancellation from the first preparation check, including every
    // later failure/unwind before either component has consumed input.
    let mut source_preparation =
        crate::parser_session_target_contract::ParserTargetPreparationGuard::new(source);
    let mut numeric_preparation =
        crate::parser_session_target_contract::ParserTargetPreparationGuard::new(numeric);
    let source = source_preparation.get();
    let numeric = numeric_preparation.get();
    let source_contract = source.storage_contract();
    let numeric_contract = numeric.storage_contract();
    let source_declared =
        crate::parser_session_target_contract::target_declared_bytes(source).ok_or(R::Pressure)?;
    let numeric_declared =
        crate::parser_session_target_contract::target_declared_bytes(numeric).ok_or(R::Pressure)?;
    let source_plan = source.original_plan_owner();
    let numeric_plan = numeric.original_plan_owner();
    let source = OwnedSourceTarget {
        target: source_preparation.release(),
        original: source_plan.clone(),
        contract: source_contract,
    };
    let numeric = OwnedNumericTarget {
        target: numeric_preparation.release(),
        contract: numeric_contract,
    };
    let live_native = family
        .borrow()
        .storage_receipt()
        .conversion_requested_bytes_bound
        .checked_mul(2)
        .ok_or(R::Pressure)?;
    let mut combined = limits.other_existing_session_reserved_bytes;
    for bytes in [
        source_declared,
        numeric_declared,
        limits
            .verification
            .preparation_peak_bytes
            .checked_mul(3)
            .ok_or(R::Pressure)?,
        limits.canonical.maximum_preparation_peak_bytes,
        limits.maximum_metadata_temporary_bytes,
        limits.maximum_plan_validation_temporary_bytes,
        limits.maximum_endpoint_encoding_requested_bytes,
        live_native,
    ] {
        combined = add(combined, bytes)?;
    }
    if combined > limits.maximum_combined_bytes || limits.maximum_invocations == 0 {
        return Err(R::Pressure);
    }
    if !crate::parser_session_target_contract::validate_shared_source_owner(&source.target)
        || !crate::parser_session_target_contract::validate_shared_source_owner(&numeric.target)
        || !P::admits_selection(selection.as_ref())
        || source.target.original_plan() != source_plan.as_ref()
        || ParserSessionExecutor::original_plan(&numeric.target) != numeric_plan.as_ref()
        || ParserNumericExecutor::plan(&numeric.target) != numeric_plan.as_ref()
    {
        return Err(R::Plan);
    }
    validate_fixed_source_plan_structure(
        source.target.expanded_source(),
        &source_plan,
        P::FEATURES,
    )
    .map_err(|_| R::Plan)?;
    crate::parser_session_numeric_plan::validate_numeric_plan_structure_for(numeric.target.expanded_source(), &numeric_plan, P::INDICES, P::SCORES)
        .map_err(|_| R::Plan)?;
    if numeric_plan_retained_bytes(&source_plan).map_err(|_| R::Pressure)?
        > source_contract.retained_bytes()
        || numeric_plan_retained_bytes(&numeric_plan).map_err(|_| R::Pressure)?
            > numeric_contract.retained_bytes()
    {
        return Err(R::Pressure);
    }
    {
        let family = family.borrow();
        for descriptor in [
            P::Query::PREPARED_DESCRIPTOR,
            P::Features::PREPARED_DESCRIPTOR,
            P::Scores::PREPARED_DESCRIPTOR,
        ] {
            if !family.contains_descriptor(descriptor) {
                return Err(R::Descriptor);
            }
        }
        verify_source_native_parity(
            source.target.checked_source(),
            &family,
            &[
                P::Query::PREPARED_DESCRIPTOR,
                P::Features::PREPARED_DESCRIPTOR,
            ],
            limits.maximum_metadata_temporary_bytes,
        )
        .map_err(|_| R::Descriptor)?;
        verify_source_native_parity(
            numeric.target.checked_source(),
            &family,
            &[
                P::Features::PREPARED_DESCRIPTOR,
                P::Scores::PREPARED_DESCRIPTOR,
            ],
            limits.maximum_metadata_temporary_bytes,
        )
        .map_err(|_| R::Descriptor)?;
    }
    let (projector, _, _, projector_receipt) =
        PreparedSourceVerification::prepare(P::INDICES, limits.verification)
            .map_err(|_| R::Source)?;
    let (wrapper, _, _, wrapper_receipt) =
        PreparedSourceVerification::prepare(P::SCORES, limits.verification)
            .map_err(|_| R::Source)?;
    let model = P::selected_model_owner(selection.as_ref()).clone();
    let model_storage = model
        .storage_receipt()
        .map_err(|_| R::Model)?
        .retained_heap_bytes_bound;
    let numeric_reservation = numeric_plan_preparation_reservation(
        &numeric_plan,
        model_storage,
        [
            projector_receipt.preparation_peak_heap_bytes_bound,
            wrapper_receipt.preparation_peak_heap_bytes_bound,
        ],
        limits.maximum_plan_validation_temporary_bytes,
    )
    .map_err(|_| R::Pressure)?;
    crate::parser_session_numeric_plan::validate_numeric_plan_seal_and_resource_for(
        numeric.target.expanded_source(),
        &numeric_plan,
        &model,
        P::INDICES,
        P::SCORES,
    )
    .map_err(|_| R::Plan)?;
    // Source preparation and its original Plan validation remain independently
    // charged; no single-program approximation replaces expanded composition.
    let source_verification =
        PreparedSourceVerification::prepare(P::FEATURES, limits.verification)
            .map_err(|_| R::Source)?;
    source_plan_preparation_reservation(
        &source_plan,
        P::FEATURES,
        source_verification.3.preparation_peak_heap_bytes_bound,
        limits.maximum_plan_validation_temporary_bytes,
    )
    .map_err(|_| R::Pressure)?;
    validate_fixed_source_plan_seal(
        source.target.expanded_source(),
        &source_plan,
        P::FEATURES,
    )
    .map_err(|_| R::Plan)?;
    let source_port = PreparedCanonicalParserSessionPort::<
        P::Query,
        P::Features,
        _,
    >::from_prepared(
        P::FEATURES,
        source,
        family.clone(),
        ParserCanonicalIngressLimits {
            maximum_invocations: limits.maximum_invocations,
            maximum_input_bytes: source_contract.maximum_input_bytes(),
            maximum_output_bytes: source_contract.maximum_output_bytes(),
        },
        limits.verification,
        limits.maximum_endpoint_encoding_requested_bytes,
        source_verification,
    )
    .map_err(|_| R::Source)?;
    let source_receipt = source_port.verification_storage();
    let canonical = PreparedCategoricalCanonicalAdmission::prepare(model, limits.canonical)
        .map_err(|_| R::Model)?;
    let canonical_storage = canonical.storage_receipt();
    let numeric_port = PreparedParserNumericCustody::<_, P>::from_prepared_with_feature_guard(
        numeric,
        family,
        projector,
        wrapper,
        canonical,
        numeric_plan,
        limits.maximum_invocations,
        selection.as_ref(),
        feature_guard,
    )
    .map_err(|_| R::Model)?;
    let owner = PreparedParserMixedCustody::from_prepared(source_port, numeric_port, source_plan);
    let retained_source = add(
        source_receipt.retained_heap_bytes_bound,
        add(
            projector_receipt.retained_heap_bytes_bound,
            wrapper_receipt.retained_heap_bytes_bound,
        )?,
    )?;
    let _ = numeric_reservation;
    Ok((
        owner,
        MixedPreparationReceipt {
            combined_declared_bytes_bound: combined,
            retained_source_bytes_bound: retained_source,
            retained_canonical_codec_bytes_bound: canonical_storage.retained_heap_bytes,
            concurrently_live_native_bytes_bound: live_native,
            target_declared_retained_bytes: add(
                source_contract.retained_bytes(),
                numeric_contract.retained_bytes(),
            )?,
            target_declared_execution_temporary_bytes: add(
                source_contract.execution_temporary_bytes(),
                numeric_contract.execution_temporary_bytes(),
            )?,
        },
    ))
}
