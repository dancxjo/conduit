//! Closed model preparation, not linguistic-quality or Session acceptance.
//! The incoming opaque selection retains the complete original model/resource
//! chain. The expected signature is freshly admitted under its original Native
//! Type and laws; identity digests do not replace that full comparison.
//! These receipts inventory dynamic ownership only. The enclosing Session
//! preparation must separately charge the exact descriptor Type/law/contract
//! artifacts and expected signature frame, deduplicated with its other static
//! resources. A family heap receipt does not include those static artifacts.
use crate::lexical_proposer_port::token_producer::model_definition::{
    PreparedProposalModelSelection, ProposalModelDefinition,
};
use crate::parser_session_window8_corrected_declaration::{
    check_fixed_declaration, EXPECTED_SIGNATURE,
};
use alloc::sync::Arc;
use conduit_ai::{
    integer_categorical_step::{
        categorical_model_metadata_storage_receipt, PreparedCategoricalStep,
    },
    ModelSignature,
};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::mem::{align_of, size_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Window8ModelRefusal {
    Declaration,
    SignatureFamily,
    Signature,
    Model,
    Overflow,
    Pressure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window8ModelLimits {
    pub(crate) maximum_existing_selection_bytes: usize,
    pub(crate) maximum_retained_model_bytes: usize,
    pub(crate) maximum_signature_conversion_requested_bytes: usize,
    pub(crate) maximum_preparation_peak_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window8ModelStorageReceipt {
    pub(crate) original_categorical_chain_bytes_bound: usize,
    pub(crate) declaration_heap_bytes_bound: usize,
    pub(crate) selection_owner_bytes_bound: usize,
    pub(crate) existing_selection_bytes_bound: usize,
    pub(crate) verified_owner_bytes_bound: usize,
    pub(crate) complete_retained_model_bytes_bound: usize,
    /// Separately prepared owner borrowed only for this admission. Its original
    /// preparation must already have been admitted by PreparedNativeFamily.
    pub(crate) borrowed_signature_family_bytes_bound: usize,
    pub(crate) signature_conversion_requested_bytes_bound: usize,
    pub(crate) preparation_peak_bytes_bound: usize,
}

/// Only successful full admission constructs this owner. It authorizes the
/// selected model seam, never a parse, commitment, played token or quality claim.
pub(crate) struct VerifiedWindow8Model {
    selection: Arc<PreparedProposalModelSelection>,
    storage: Window8ModelStorageReceipt,
}

fn add(a: usize, b: usize) -> Result<usize, Window8ModelRefusal> {
    a.checked_add(b).ok_or(Window8ModelRefusal::Overflow)
}
fn arc_bytes<T>() -> Result<usize, Window8ModelRefusal> {
    add(
        add(size_of::<T>(), 2 * size_of::<usize>())?,
        4 * align_of::<T>(),
    )
}

/// Allocation-free actual incoming storage inventory. All boxed declaration
/// buffers are exact-sized; full signature/metadata capacities are counted by
/// the generic model owner's allocation-free inventory.
fn incoming_storage(
    selection: &PreparedProposalModelSelection,
) -> Result<Window8ModelStorageReceipt, Window8ModelRefusal> {
    use Window8ModelRefusal as R;
    let d = selection.declaration();
    let model = selection
        .categorical()
        .storage_receipt()
        .map_err(|_| R::Model)?
        .retained_heap_bytes_bound;
    let metadata = categorical_model_metadata_storage_receipt(d.artifact(), d.signature())
        .map_err(|_| R::Model)?;
    // The metadata inline fields are already inside ProposalModelDefinition.
    let mut declaration = add(
        arc_bytes::<ProposalModelDefinition>()?,
        metadata.owned_heap_bytes_bound,
    )?;
    for bytes in [
        d.identity().len(),
        d.feature_abi().len(),
        d.canonical_proposer_definition().len(),
        d.training_manifest().len(),
    ] {
        declaration = add(declaration, bytes)?;
    }
    let selection_owner = arc_bytes::<PreparedProposalModelSelection>()?;
    let existing = add(add(model, declaration)?, selection_owner)?;
    let verified_owner = arc_bytes::<VerifiedWindow8Model>()?;
    Ok(Window8ModelStorageReceipt {
        original_categorical_chain_bytes_bound: model,
        declaration_heap_bytes_bound: declaration,
        selection_owner_bytes_bound: selection_owner,
        existing_selection_bytes_bound: existing,
        verified_owner_bytes_bound: verified_owner,
        complete_retained_model_bytes_bound: add(existing, verified_owner)?,
        borrowed_signature_family_bytes_bound: 0,
        signature_conversion_requested_bytes_bound: 0,
        preparation_peak_bytes_bound: 0,
    })
}

impl VerifiedWindow8Model {
    pub(crate) fn admit(
        selection: Arc<PreparedProposalModelSelection>,
        signature_family: &mut PreparedNativeFamily,
        limits: Window8ModelLimits,
    ) -> Result<Arc<Self>, Window8ModelRefusal> {
        use Window8ModelRefusal as R;
        let mut storage = incoming_storage(&selection)?;
        let family = signature_family.storage_receipt();
        storage.borrowed_signature_family_bytes_bound = family.retained_heap_bytes_bound;
        storage.signature_conversion_requested_bytes_bound =
            family.conversion_requested_bytes_bound;
        storage.preparation_peak_bytes_bound = add(
            storage.complete_retained_model_bytes_bound,
            add(
                family.retained_heap_bytes_bound,
                family.conversion_requested_bytes_bound,
            )?,
        )?;
        if storage.existing_selection_bytes_bound > limits.maximum_existing_selection_bytes
            || storage.complete_retained_model_bytes_bound > limits.maximum_retained_model_bytes
            || family.conversion_requested_bytes_bound
                > limits.maximum_signature_conversion_requested_bytes
            || storage.preparation_peak_bytes_bound > limits.maximum_preparation_peak_bytes
        {
            return Err(R::Pressure);
        }
        check_fixed_declaration(selection.declaration()).map_err(|_| R::Declaration)?;
        if !signature_family.contains_descriptor(ModelSignature::PREPARED_DESCRIPTOR) {
            return Err(R::SignatureFamily);
        }
        {
            // The family enforces complete generic canonical validation,
            // recursive child conversion and every original Native law.
            let expected = signature_family
                .decode::<ModelSignature>(EXPECTED_SIGNATURE)
                .map_err(|_| R::Signature)?;
            if &expected != selection.declaration().signature()
                || &expected != selection.categorical().resource().signature()
                || selection.declaration().artifact()
                    != selection.categorical().resource().artifact()
            {
                return Err(R::Signature);
            }
        }
        // No expected-signature clone is retained. Its original complete static
        // frame and the complete original immutable selection remain available.
        Ok(Arc::new(Self { selection, storage }))
    }
    pub(crate) fn selection(&self) -> &PreparedProposalModelSelection {
        &self.selection
    }
    pub(crate) fn categorical(&self) -> &PreparedCategoricalStep {
        self.selection.categorical()
    }
    pub(crate) fn declaration(&self) -> &ProposalModelDefinition {
        self.selection.declaration()
    }
    pub(crate) fn storage_receipt(&self) -> Window8ModelStorageReceipt {
        self.storage
    }
    pub(crate) fn expected_signature_frame(&self) -> &'static [u8] {
        EXPECTED_SIGNATURE
    }
}
