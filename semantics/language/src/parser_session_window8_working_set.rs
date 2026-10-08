//! Fixed working-set construction. All simultaneous declared ceilings are
//! admitted before the first family allocation. The enclosing Session adds its
//! original model/proposer/target owners and finite revision books; this helper
//! neither admits a revision nor publishes facts.
use crate::{
    parser_canonical_u64_collection::PreparedParserU64Collection,
    parser_session_pure_source::{PreparedParserPureSource, PureSourceLimits},
    parser_session_window8_ancestry::PreparedWindow8Ancestry,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8AtomLimits},
    parser_session_window8_families::{Window8Families, Window8FamilyLimits},
    parser_session_window8_numeric_family::Window8NumericFamily,
    parser_session_window8_ports::{self, PORTS, family_for},
    parser_session_window8_queries::{PreparedWindow8Queries, Window8QueryPreparationLimits},
    parser_session_window8_snapshot::PreparedSnapshotFrames,
    parser_session_window8_source_profile,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{
    NativeFamilyTypeDescriptor, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkingSetRefusal;
#[derive(Clone, Copy)]
pub(crate) enum Window8HistoryOwnership {
    IndependentPortCeilings,
    /// Every revision must be reserved through prepare_book below. The one
    /// complete linked-book ceiling includes all Source/model/admission frames.
    SharedRevisionBooks(crate::parser_session_window8_book::BookLimits),
}
#[derive(Clone, Copy)]
pub(crate) struct Window8WorkingSetLimits {
    pub(crate) families: Window8FamilyLimits,
    pub(crate) numeric_family: PreparedNativeFamilyLimits,
    pub(crate) source: PureSourceLimits,
    pub(crate) queries: Window8QueryPreparationLimits,
    pub(crate) atoms: Window8AtomLimits,
    pub(crate) maximum_atom_static_bytes: usize,
    pub(crate) maximum_ancestry_preparation_bytes: usize,
    pub(crate) maximum_ancestry_retained_bytes: usize,
    pub(crate) maximum_choices_bytes: usize,
    pub(crate) maximum_tokens_collection_bytes: usize,
    pub(crate) maximum_snapshot_bytes: usize,
    pub(crate) observation_candidates: usize,
    pub(crate) maximum_observation_pool_bytes: usize,
    pub(crate) fact: crate::parser_canonical_composition::ParserCompositionLimits,
    pub(crate) refinement: crate::parser_canonical_refinement::ParserRefinementLimits,
    pub(crate) maximum_source_preparation_bytes: usize,
    pub(crate) maximum_source_retained_bytes: usize,
    pub(crate) maximum_source_history_bytes: usize,
    pub(crate) history: Window8HistoryOwnership,
    pub(crate) existing_owner_bytes: usize,
    pub(crate) maximum_complete_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkingSetReceipt {
    pub(crate) complete_declared_bytes_bound: usize,
    pub(crate) new_preparation_bytes_bound: usize,
    pub(crate) new_retained_bytes_bound: usize,
    pub(crate) source_history_bytes_bound: usize,
    pub(crate) sum_of_port_history_limits: usize,
}
pub(crate) struct PreparedWindow8WorkingSet {
    pub(crate) families: Window8Families,
    pub(crate) numeric_family: Window8NumericFamily,
    pub(crate) source: Vec<PreparedParserPureSource>,
    pub(crate) queries: PreparedWindow8Queries,
    pub(crate) atoms: PreparedWindow8Atoms,
    pub(crate) ancestry: PreparedWindow8Ancestry,
    pub(crate) choices: PreparedParserU64Collection,
    pub(crate) tokens: crate::parser_session_window8_collection::PreparedWindow8CanonicalCollection,
    pub(crate) snapshots: PreparedSnapshotFrames,
    pub(crate) observations: crate::parser_session_window8_observe::PreparedObservationFrames,
    pub(crate) fact: crate::parser_canonical_composition::PreparedParserCanonicalComposer,
    pub(crate) refinement: crate::parser_canonical_refinement::PreparedParserCanonicalRefinement<
        crate::generated::LanguageParserWindow8QualifiedLexicalProposal,
        crate::generated::LanguageParserWindow8QualifiedLexicalAnalysis,
    >,
    pub(crate) observation_family:
        alloc::rc::Rc<core::cell::RefCell<conduit_plot::rust_binding::PreparedNativeFamily>>,
    pub(crate) receipt: WorkingSetReceipt,
    history: Window8HistoryOwnership,
}
fn add(a: usize, b: usize) -> Result<usize, WorkingSetRefusal> {
    a.checked_add(b).ok_or(WorkingSetRefusal)
}
impl PreparedWindow8WorkingSet {
    /// Normal shared-history entrance. Callers cannot substitute per-revision
    /// limits after aggregate preparation; every linked book uses this profile.
    pub(crate) fn prepare_book(
        &self,
        previous: Option<alloc::rc::Rc<crate::parser_session_window8_book::Window8Book>>,
        model: alloc::sync::Arc<crate::parser_session_window8_model::VerifiedWindow8Model>,
        proposer: alloc::rc::Rc<
            core::cell::RefCell<
                crate::lexical_proposer_port::token_producer::revision::PreparedRevisionProducer,
            >,
        >,
    ) -> Result<crate::parser_session_window8_book::Window8Book, WorkingSetRefusal> {
        let Window8HistoryOwnership::SharedRevisionBooks(limits) = self.history else {
            return Err(WorkingSetRefusal);
        };
        crate::parser_session_window8_book::Window8Book::reserve(limits, previous, model, proposer)
            .map_err(|_| WorkingSetRefusal)
    }
    pub(crate) fn reservation(
        limits: &Window8WorkingSetLimits,
    ) -> Result<WorkingSetReceipt, WorkingSetRefusal> {
        Ok(Self::preflight(limits)?.0)
    }
    fn preflight(
        limits: &Window8WorkingSetLimits,
    ) -> Result<
        (
            WorkingSetReceipt,
            crate::parser_session_window8_numeric_family::NumericFamilyReservation,
            crate::parser_session_window8_source_profile::SourceProfileReservation,
            usize,
            usize,
        ),
        WorkingSetRefusal,
    > {
        if limits.observation_candidates == 0
            || PORTS.len() != 54
            || limits.queries.maximum_frame_bytes != limits.atoms.maximum_frame_bytes
        {
            return Err(WorkingSetRefusal);
        }
        let numeric = Window8NumericFamily::reservation(limits.numeric_family)
            .map_err(|_| WorkingSetRefusal)?;
        let mut family_retained = 0usize;
        let mut family_preparation = 0usize;
        let mut conversion = 0usize;
        // Complete four-family limits already include their allocation owners.
        // Their aggregate limits are upper bounds checked again by prepare.
        for family in limits.families.families {
            family_retained = add(family_retained, family.maximum_retained_bytes)?;
            family_preparation = add(family_preparation, family.maximum_preparation_peak_bytes)?;
            conversion = conversion.max(family.maximum_conversion_requested_bytes);
        }
        family_retained = family_retained.max(limits.families.maximum_retained_bytes);
        family_preparation = family_preparation.max(limits.families.maximum_preparation_peak_bytes);
        let active = conversion
            .checked_mul(limits.families.concurrent_native_values.max(2))
            .ok_or(WorkingSetRefusal)?;
        let source = parser_session_window8_source_profile::reserve_all(
            &[family_retained; 54],
            &[active; 54],
            limits.source,
            limits.maximum_source_preparation_bytes,
            limits.maximum_source_retained_bytes,
            match limits.history {
                Window8HistoryOwnership::IndependentPortCeilings => {
                    limits.maximum_source_history_bytes
                }
                Window8HistoryOwnership::SharedRevisionBooks(_) => usize::MAX,
            },
        )
        .map_err(|_| WorkingSetRefusal)?;
        let charged_history = match limits.history {
            Window8HistoryOwnership::IndependentPortCeilings => source.history_bytes,
            Window8HistoryOwnership::SharedRevisionBooks(book) => {
                if book.maximum_history_books == 0
                    || book.maximum_all_history_retained_bytes == 0
                    || book.maximum_all_history_retained_bytes > limits.maximum_source_history_bytes
                {
                    return Err(WorkingSetRefusal);
                }
                book.maximum_all_history_retained_bytes
            }
        };
        let mut preparation = add(family_preparation, numeric.preparation_bytes)?;
        let mut retained = add(family_retained, numeric.retained_bytes)?;
        for (p, r) in [
            (source.preparation_bytes, source.retained_bytes),
            (
                limits.queries.maximum_preparation_requested_bytes,
                limits.queries.maximum_retained_requested_bytes,
            ),
            (
                limits.atoms.maximum_preparation_requested_bytes,
                limits.atoms.maximum_retained_requested_bytes,
            ),
            (
                limits.maximum_ancestry_preparation_bytes,
                limits.maximum_ancestry_retained_bytes,
            ),
            (limits.maximum_choices_bytes, limits.maximum_choices_bytes),
            (
                limits.maximum_tokens_collection_bytes,
                limits.maximum_tokens_collection_bytes,
            ),
            (limits.maximum_snapshot_bytes, limits.maximum_snapshot_bytes),
            (
                limits.maximum_observation_pool_bytes,
                limits.maximum_observation_pool_bytes,
            ),
            (
                limits.fact.maximum_preparation_requested_bytes,
                limits.fact.maximum_retained_requested_bytes,
            ),
            (
                limits.refinement.maximum_preparation_requested_bytes,
                limits.refinement.maximum_retained_requested_bytes,
            ),
        ] {
            preparation = add(preparation, p)?;
            retained = add(retained, r)?;
        }
        preparation = add(preparation, core::mem::size_of::<Self>())?;
        retained = add(retained, core::mem::size_of::<Self>())?;
        let static_bytes = add(
            limits.families.maximum_static_bytes,
            limits.maximum_atom_static_bytes,
        )?;
        let complete = add(
            add(
                add(
                    add(preparation.max(retained), static_bytes)?,
                    charged_history,
                )?,
                active.max(numeric.conversion_bytes),
            )?,
            limits.existing_owner_bytes,
        )?;
        if complete > limits.maximum_complete_bytes {
            return Err(WorkingSetRefusal);
        }
        // No allocation occurred above. External owners and future history are
        // charged in the same simultaneous envelope before family construction.
        let family_share = add(
            add(
                family_preparation.max(family_retained),
                limits.families.maximum_static_bytes,
            )?,
            conversion
                .checked_mul(limits.families.concurrent_native_values)
                .ok_or(WorkingSetRefusal)?,
        )?;
        Ok((
            WorkingSetReceipt {
                complete_declared_bytes_bound: complete,
                new_preparation_bytes_bound: preparation,
                new_retained_bytes_bound: retained,
                source_history_bytes_bound: charged_history,
                sum_of_port_history_limits: source.history_bytes,
            },
            numeric,
            source,
            active,
            family_share,
        ))
    }
    pub(crate) fn prepare(
        mut limits: Window8WorkingSetLimits,
        extra_static_roots: &[&'static NativeFamilyTypeDescriptor],
    ) -> Result<Self, WorkingSetRefusal> {
        let (receipt, numeric, source, active, family_share) = Self::preflight(&limits)?;
        let complete = receipt.complete_declared_bytes_bound;
        limits.families.other_reserved_bytes = complete
            .checked_sub(family_share)
            .ok_or(WorkingSetRefusal)?;
        limits.families.maximum_combined_bytes = complete;
        let families = Window8Families::prepare(limits.families, extra_static_roots)
            .map_err(|_| WorkingSetRefusal)?;
        let numeric_family = Window8NumericFamily::prepare(
            limits.numeric_family,
            numeric.preparation_bytes,
            numeric.retained_bytes,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let mut policies = [limits.source; 54];
        for (index, policy) in policies.iter_mut().enumerate() {
            *policy = parser_session_window8_source_profile::limits(index, *policy)
                .map_err(|_| WorkingSetRefusal)?;
        }
        let source_owners = parser_session_window8_ports::prepare(
            &families.owners,
            &policies,
            add(
                add(add(source.retained_bytes, source.history_bytes)?, active)?,
                source.preparation_bytes,
            )?,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let queries = PreparedWindow8Queries::prepare(&families.owners, limits.queries)
            .map_err(|_| WorkingSetRefusal)?;
        let atoms = PreparedWindow8Atoms::prepare(&families.owners, limits.atoms)
            .map_err(|_| WorkingSetRefusal)?;
        if atoms.receipt().static_selector_bytes_bound > limits.maximum_atom_static_bytes {
            return Err(WorkingSetRefusal);
        }
        let descriptor = crate::generated::LanguageParserWindow8StateProof::PREPARED_DESCRIPTOR;
        let index = family_for(&families.owners, descriptor).map_err(|_| WorkingSetRefusal)?;
        let ancestry = PreparedWindow8Ancestry::prepare(
            &*families.owners[index]
                .try_borrow()
                .map_err(|_| WorkingSetRefusal)?,
            limits.queries.maximum_frame_bytes,
            limits.maximum_ancestry_preparation_bytes,
            limits.maximum_ancestry_retained_bytes,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let descriptor = crate::generated::LanguageParserWindow8RawHypothesis::PREPARED_DESCRIPTOR;
        let index = family_for(&families.owners, descriptor).map_err(|_| WorkingSetRefusal)?;
        let choices = PreparedParserU64Collection::prepare::<
            crate::generated::LanguageParserWindow8RawHypothesis,
        >(
            &*families.owners[index]
                .try_borrow()
                .map_err(|_| WorkingSetRefusal)?,
            &["choices"],
            limits.maximum_choices_bytes,
            limits.maximum_choices_bytes,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let index = family_for(
            &families.owners,
            crate::generated::LanguageParserWindow8RawProjection::PREPARED_DESCRIPTOR,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let tokens=crate::parser_session_window8_collection::PreparedWindow8CanonicalCollection::prepare::<crate::generated::LanguageParserWindow8RawProjection>(
            &*families.owners[index].try_borrow().map_err(|_|WorkingSetRefusal)?,&["tokens"],
            limits.maximum_tokens_collection_bytes,limits.maximum_tokens_collection_bytes,limits.maximum_tokens_collection_bytes)
            .map_err(|_|WorkingSetRefusal)?;
        let snapshots = PreparedSnapshotFrames::prepare(
            limits.queries.maximum_frame_bytes,
            limits.maximum_snapshot_bytes,
            limits.maximum_snapshot_bytes,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let observations =
            crate::parser_session_window8_observe::PreparedObservationFrames::prepare(
                limits.observation_candidates,
                limits.queries.maximum_frame_bytes,
                limits.maximum_observation_pool_bytes,
            )
            .map_err(|_| WorkingSetRefusal)?;
        let index = family_for(
            &families.owners,
            crate::generated::LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR,
        )
        .map_err(|_| WorkingSetRefusal)?;
        let observation_family = alloc::rc::Rc::clone(&families.owners[index]);
        let family = observation_family
            .try_borrow()
            .map_err(|_| WorkingSetRefusal)?;
        let fact = crate::parser_canonical_composition::PreparedParserCanonicalComposer::prepare::<
            crate::generated::LanguageParserWindow8FactQuery,
        >(&*family, limits.fact)
        .map_err(|_| WorkingSetRefusal)?;
        let refinement = crate::parser_canonical_refinement::PreparedParserCanonicalRefinement::<
            crate::generated::LanguageParserWindow8QualifiedLexicalProposal,
            crate::generated::LanguageParserWindow8QualifiedLexicalAnalysis,
        >::prepare(&*family, limits.refinement)
        .map_err(|_| WorkingSetRefusal)?;
        drop(family);
        Ok(Self {
            families,
            numeric_family,
            source: source_owners,
            queries,
            atoms,
            ancestry,
            choices,
            tokens,
            snapshots,
            observations,
            fact,
            refinement,
            observation_family,
            receipt,
            history: limits.history,
        })
    }
}
