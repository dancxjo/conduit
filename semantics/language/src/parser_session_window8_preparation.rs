//! Whole fixed Window8 preparation from already-owned, freshly verified model
//! and proposer inputs. External target storage remains an explicit contract;
//! these declarations are included, never presented as measured target heap.
//! The public Session must additionally reserve finite books before revisions.
use crate::{
    lexical_proposer_port::token_producer::revision::PreparedRevisionProducer,
    parser_canonical_composition::ParserCompositionLimits,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_feature_guard::PreparedParserFeatureGuard,
    parser_session_mixed_preparation::{
        self, MixedPreparationLimits, MixedPreparationReceipt, OwnedNumericTarget,
        OwnedSourceTarget,
    },
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_target_contract::{
        ParserSessionPreparedTarget, ParserTargetPreparationGuard, target_declared_bytes,
    },
    parser_session_window8_model::VerifiedWindow8Model,
    parser_session_window8_numeric_profile::ProposalWindow8V2NumericProfile,
    parser_session_window8_registry::Window8Registry,
    parser_session_window8_working_set::{PreparedWindow8WorkingSet, Window8WorkingSetLimits},
};
use alloc::{rc::Rc, sync::Arc};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
use core::{
    cell::RefCell,
    mem::{align_of, size_of},
};
#[derive(Debug)]
pub(crate) enum Window8PreparationRefusal {
    Contract,
    WorkingSet(crate::parser_session_window8_working_set::WorkingSetRefusal),
    Mixed(crate::parser_session_mixed_preparation::MixedPreparationRefusal),
}
pub(crate) struct PreparedWindow8Factory<
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget + ParserNumericExecutor,
> {
    pub(crate) registry: Window8Registry<OwnedSourceTarget<S>, OwnedNumericTarget<N>>,
    pub(crate) working: PreparedWindow8WorkingSet,
    pub(crate) model: Arc<VerifiedWindow8Model>,
    pub(crate) proposer: Rc<RefCell<PreparedRevisionProducer>>,
    pub(crate) mixed_receipt: MixedPreparationReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, Window8PreparationRefusal> {
    a.checked_add(b).ok_or(Window8PreparationRefusal::Contract)
}
impl<S, N> PreparedWindow8Factory<S, N>
where
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget
        + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    /// Full closed entry: original selection metadata and all subsequent owner
    /// ceilings are checked before even the signature-family preparation.
    pub(crate) fn prepare(
        source: S,
        numeric: N,
        selection:Arc<crate::lexical_proposer_port::token_producer::model_definition::PreparedProposalModelSelection>,
        proposer: Rc<RefCell<PreparedRevisionProducer>>,
        working_limits: Window8WorkingSetLimits,
        mixed_limits: MixedPreparationLimits,
        guard_limits: ParserCompositionLimits,
        signature_limits: conduit_plot::rust_binding::PreparedNativeFamilyLimits,
        model_limits: crate::parser_session_window8_model::Window8ModelLimits,
    ) -> Result<Self, Window8PreparationRefusal> {
        let mut source_guard = ParserTargetPreparationGuard::new(source);
        let mut numeric_guard = ParserTargetPreparationGuard::new(numeric);
        crate::parser_session_window8_corrected_declaration::check_fixed_declaration(
            selection.declaration(),
        )
        .map_err(|_| Window8PreparationRefusal::Contract)?;
        let incoming = crate::parser_session_window8_model::incoming_storage(&selection)
            .map_err(|_| Window8PreparationRefusal::Contract)?;
        let producer = proposer
            .try_borrow()
            .map_err(|_| Window8PreparationRefusal::Contract)?;
        if producer.canonical_proposer_definition()
            != selection.declaration().canonical_proposer_definition()
        {
            return Err(Window8PreparationRefusal::Contract);
        }
        let producer_owner = add(
            add(
                size_of::<RefCell<PreparedRevisionProducer>>(),
                2 * size_of::<usize>(),
            )?,
            4 * align_of::<RefCell<PreparedRevisionProducer>>(),
        )?;
        let mut existing = add(incoming.complete_retained_model_bytes_bound, producer_owner)?;
        existing = add(existing, producer.storage_receipt().retained_bytes_bound)?;
        existing = add(
            existing,
            producer.storage_receipt().admission_peak_bytes_bound,
        )?;
        existing = add(
            existing,
            target_declared_bytes(source_guard.get()).ok_or(Window8PreparationRefusal::Contract)?,
        )?;
        existing = add(
            existing,
            target_declared_bytes(numeric_guard.get()).ok_or(Window8PreparationRefusal::Contract)?,
        )?;
        existing = add(existing, mixed_limits.maximum_combined_bytes)?;
        existing = add(
            existing,
            guard_limits
                .maximum_preparation_requested_bytes
                .max(guard_limits.maximum_retained_requested_bytes),
        )?;
        existing = add(
            existing,
            signature_limits
                .maximum_preparation_peak_bytes
                .max(signature_limits.maximum_retained_bytes),
        )?;
        existing = add(
            existing,
            signature_limits.maximum_conversion_requested_bytes,
        )?;
        existing = add(
            existing,
            core::mem::size_of::<conduit_plot::rust_binding::PreparedNativeFamily>(),
        )?;
        existing = add(
            existing,
            crate::parser_session_window8_corrected_declaration::EXPECTED_SIGNATURE.len(),
        )?;
        existing = add(existing, size_of::<Self>())?;
        let mut preflight = working_limits;
        preflight.existing_owner_bytes = add(preflight.existing_owner_bytes, existing)?;
        PreparedWindow8WorkingSet::reservation(&preflight)
            .map_err(|_| Window8PreparationRefusal::Contract)?;
        let resources = crate::parser_session_window8_static::descriptor_resources(&[
            conduit_ai::ModelSignature::PREPARED_DESCRIPTOR,
        ])
        .ok_or(Window8PreparationRefusal::Contract)?;
        let static_bytes = add(
            add(
                resources.canonical_bytes_bound,
                resources.descriptor_storage_bytes_bound,
            )?,
            resources.source_canonical_bytes_bound,
        )?;
        if static_bytes > working_limits.families.maximum_static_bytes {
            return Err(Window8PreparationRefusal::Contract);
        }
        drop(producer);
        let mut signature = conduit_plot::rust_binding::PreparedNativeFamily::prepare(
            &[conduit_ai::ModelSignature::PREPARED_DESCRIPTOR],
            signature_limits,
        )
        .map_err(|_| Window8PreparationRefusal::Contract)?;
        let model = VerifiedWindow8Model::admit(selection, &mut signature, model_limits)
            .map_err(|_| Window8PreparationRefusal::Contract)?;
        drop(signature);
        Self::prepare_verified(
            source_guard.release(),
            numeric_guard.release(),
            model,
            proposer,
            working_limits,
            mixed_limits,
            guard_limits,
        )
    }
    /// Inputs were prepared outside this constructor; their full retained chains
    /// are inventoried before any new family allocation or ingress consumption.
    pub(crate) fn prepare_verified(
        source: S,
        numeric: N,
        model: Arc<VerifiedWindow8Model>,
        proposer: Rc<RefCell<PreparedRevisionProducer>>,
        mut working_limits: Window8WorkingSetLimits,
        mixed_limits: MixedPreparationLimits,
        guard_limits: ParserCompositionLimits,
    ) -> Result<Self, Window8PreparationRefusal> {
        let mut source_guard = ParserTargetPreparationGuard::new(source);
        let mut numeric_guard = ParserTargetPreparationGuard::new(numeric);
        let producer = proposer
            .try_borrow()
            .map_err(|_| Window8PreparationRefusal::Contract)?;
        if producer.canonical_proposer_definition()
            != model.declaration().canonical_proposer_definition()
        {
            return Err(Window8PreparationRefusal::Contract);
        }
        let receipt = producer.storage_receipt();
        let producer_owner = add(
            add(
                size_of::<RefCell<PreparedRevisionProducer>>(),
                2 * size_of::<usize>(),
            )?,
            4 * align_of::<RefCell<PreparedRevisionProducer>>(),
        )?;
        let mut existing = add(receipt.retained_bytes_bound, producer_owner)?;
        existing = add(existing, receipt.admission_peak_bytes_bound)?;
        existing = add(
            existing,
            model.storage_receipt().complete_retained_model_bytes_bound,
        )?;
        existing = add(existing, model.expected_signature_frame().len())?;
        existing = add(
            existing,
            target_declared_bytes(source_guard.get()).ok_or(Window8PreparationRefusal::Contract)?,
        )?;
        existing = add(
            existing,
            target_declared_bytes(numeric_guard.get()).ok_or(Window8PreparationRefusal::Contract)?,
        )?;
        // The mixed constructor separately proves all metadata/Plan/codec
        // ceilings. Admit its entire declared ceiling before any earlier owner.
        existing = add(existing, mixed_limits.maximum_combined_bytes)?;
        existing = add(
            existing,
            guard_limits
                .maximum_preparation_requested_bytes
                .max(guard_limits.maximum_retained_requested_bytes),
        )?;
        existing = add(existing, size_of::<Self>())?;
        working_limits.existing_owner_bytes = add(working_limits.existing_owner_bytes, existing)?;
        drop(producer);
        let mut working = PreparedWindow8WorkingSet::prepare(
            working_limits,
            &[conduit_ai::ModelSignature::PREPARED_DESCRIPTOR],
        )
        .map_err(Window8PreparationRefusal::WorkingSet)?;
        let family = Rc::clone(&working.numeric_family.owner);
        let guard = PreparedParserFeatureGuard::prepare(
            &*family.try_borrow().map_err(|_| Window8PreparationRefusal::Contract)?,
            crate::generated::LanguageParserProposalWindow8V2RawFeatures::PREPARED_DESCRIPTOR,
            crate::generated::LanguageParserProposalWindow8V2Features::PREPARED_DESCRIPTOR,
            guard_limits,
        )
        .map_err(|_| Window8PreparationRefusal::Contract)?;
        let (mixed, mixed_receipt) = parser_session_mixed_preparation::prepare_mixed_targets_for::<
            ProposalWindow8V2NumericProfile,
            _,
            _,
        >(
            source_guard.release(),
            numeric_guard.release(),
            Arc::clone(&model),
            family,
            mixed_limits,
            Some(guard),
        )
        .map_err(Window8PreparationRefusal::Mixed)?;
        let source = core::mem::take(&mut working.source);
        let registry =
            Window8Registry::from_prepared(source, mixed).map_err(|_| Window8PreparationRefusal::Contract)?;
        Ok(Self {
            registry,
            working,
            model,
            proposer,
            mixed_receipt,
        })
    }
}
