//! Private extraction checkpoints. These tests do not assert public Session acceptance.
#![cfg(feature = "parser-model-selection")]
#![allow(dead_code, unused_attributes)]
extern crate alloc;
#[path = "../src/lib.rs"]
mod language;
pub use language::revision;
pub use language::*;
#[path = "../src/parser_session_fixed_ingress.rs"]
mod parser_session_fixed_ingress;
#[path = "../src/parser_session_historical_base.rs"]
mod parser_session_historical_base;
#[path = "../src/parser_session_mixed_custody.rs"]
mod parser_session_mixed_custody;
#[path = "../src/parser_session_numeric_custody.rs"]
mod parser_session_numeric_custody;
#[path = "../src/parser_session_numeric_plan_storage.rs"]
mod parser_session_numeric_plan_storage;
#[path = "../src/parser_session_profile.rs"]
mod parser_session_profile;
#[path = "../src/parser_session_revision_custody.rs"]
mod parser_session_revision_custody;

#[path = "../src/parser_session_stage.rs"]
mod parser_session_stage;

#[path = "../src/parser_session_target_registry.rs"]
mod parser_session_target_registry;

#[path = "../src/parser_session_candidate_admission.rs"]
mod parser_session_candidate_admission;

#[path = "../src/parser_canonical_schema.rs"]
mod parser_canonical_schema;

#[path = "../src/parser_canonical_composition.rs"]
mod parser_canonical_composition;

#[path = "../src/parser_canonical_refinement.rs"]
mod parser_canonical_refinement;

#[path = "../src/parser_session_revision_stage.rs"]
mod parser_session_revision_stage;

#[path = "../src/parser_session_fixed_bindings.rs"]
mod parser_session_fixed_bindings;

#[path = "../src/parser_session_target_contract.rs"]
mod parser_session_target_contract;

#[path = "../src/parser_session_source_plan_storage.rs"]
mod parser_session_source_plan_storage;

#[path = "../src/parser_production_families.rs"]
mod parser_production_families;

#[path = "../src/parser_source_native_parity.rs"]
mod parser_source_native_parity;

#[path = "../src/parser_session_source_plan.rs"]
mod parser_session_source_plan;

#[path = "../src/parser_session_fixed_preparation.rs"]
mod parser_session_fixed_preparation;

#[path = "../src/parser_session_queries.rs"]
mod parser_session_queries;

#[path = "../src/parser_canonical_nominal.rs"]
mod parser_canonical_nominal;

#[path = "../src/parser_session_seed_admission.rs"]
mod parser_session_seed_admission;

#[path = "../src/parser_session_numeric_plan.rs"]
mod parser_session_numeric_plan;

#[path = "../src/parser_session_mixed_preparation.rs"]
mod parser_session_mixed_preparation;

extern crate conduitos as actual_expression_owner;
use parser_session_numeric_custody as numeric_custody;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_production_runtime.rs"]
mod runtime;
use conduit_ai::integer_categorical_step::PreparedCategoricalStep;
use conduit_plot::rust_binding::*;
use parser_production_families as families;
use std::{rc::Rc, sync::Arc};
fn source() -> String {
    [
        include_str!("../types.conduit"),
        include_str!("../identity.conduit"),
        include_str!("../coverage.conduit"),
        include_str!("../syntax.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_window8.conduit"),
        include_str!("../parser_window8_search.conduit"),
        include_str!("../parser_window8_facts.conduit"),
        include_str!("../parser_window8_lexical_selection.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_scorer.conduit"),
        include_str!("../parser_mask.conduit"),
        include_str!("../parser_joint.conduit"),
        include_str!("../parser_session_seed.conduit"),
        include_str!("../parser_session_completion.conduit"),
        include_str!("../parser_numeric_ports.conduit"),
        include_str!("../parser_session_numeric.conduit"),
        include_str!("../parser_joint_decode.conduit"),
        include_str!("../parser_available.conduit"),
        include_str!("../parser_revision.conduit"),
        include_str!("../parser_scorer_v2.conduit"),
        include_str!("../parser_session.conduit"),
        include_str!("../parser_session_policy.conduit"),
        include_str!("../parser_session_facts.conduit"),
        include_str!("../parser_session_dependency.conduit"),
        include_str!("../parser_session_committed_dependency.conduit"),
        include_str!("../parser_session_commit.conduit"),
        include_str!("../parser_session_rebase.conduit"),
        include_str!("../parser_session_branch.conduit"),
        include_str!("../parser_session_protection.conduit"),
        include_str!("../parser_session_protected_set.conduit"),
        include_str!("../parser_session_independent_admission.conduit"),
        include_str!("../parser_session_protected_rebase.conduit"),
        include_str!("../parser_session_protected_forest.conduit"),
        include_str!("../parser_session_protected_branch.conduit"),
        include_str!("../parser_session_protected_mask.conduit"),
        include_str!("../parser_session_custody.conduit"),
        include_str!("../parser_session_custody_origins.conduit"),
        include_str!("../parser_session_custody_frontier.conduit"),
        include_str!("../parser_session_custody_initialize.conduit"),
        include_str!("../parser_session_independent_commit.conduit"),
        include_str!("../parser_session_independent_commit_rebase.conduit"),
        include_str!("../discourse.conduit"),
        include_str!("../prosody.conduit"),
        include_str!("../pronunciation_selection.conduit"),
    ]
    .join("\n")
}

#[test]
fn complete_fixed_query_bank_preserves_native_input_and_refuses_aggregate_pressure() {
    use parser_session_execution::ParserSessionEntry as Entry;
    use parser_session_queries::*;
    let families = families::PreparedProductionParserFamilies::prepare(family_limits()).unwrap();
    let limits = ParserQueryPreparationLimits {
        maximum_frame_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_preparation_requested_bytes: 16 * 1024 * 1024 * 1024,
        maximum_retained_requested_bytes: 16 * 1024 * 1024 * 1024,
    };
    let mut bank = PreparedParserSessionQueries::prepare(&families, limits).unwrap();
    let receipt = bank.receipt();
    assert!(receipt.preparation_requested_bytes_bound >= receipt.retained_requested_bytes_bound);
    for refused in [
        ParserQueryPreparationLimits {
            maximum_preparation_requested_bytes: receipt.preparation_requested_bytes_bound - 1,
            ..limits
        },
        ParserQueryPreparationLimits {
            maximum_retained_requested_bytes: receipt.retained_requested_bytes_bound - 1,
            ..limits
        },
    ] {
        assert!(matches!(
            PreparedParserSessionQueries::prepare(&families, refused),
            Err(ParserQueryRefusal::Pressure)
        ));
    }
    // Ordinary encoding is only the independent fixture oracle. Production
    // composition borrows the complete exact canonical fields and allocates no
    // input Type or Native metadata during a revision.
    let query = query();
    let input = LanguageParserAvailableState::new(query.lexical().clone(), query.state().clone())
        .unwrap()
        .encode()
        .unwrap();
    let value = conduit_core::validate_canonical_structured_value(&input).unwrap();
    let (descriptor, _) = families::port_descriptors(Entry::WaitState).unwrap();
    let parser_canonical_schema::Shape::Record(fields) =
        parser_canonical_schema::shape(descriptor.type_bytes).unwrap()
    else {
        panic!("fixed wait query must be a record")
    };
    let mut selected = [value; 32];
    let mut count = 0;
    for field in fields {
        let (name, _) = field.unwrap();
        selected[count] = value.record_field(name).unwrap().unwrap();
        count += 1;
    }
    assert_eq!(
        bank.record(Entry::WaitState, &selected[..count]).unwrap(),
        input
    );
    assert!(matches!(
        bank.record(Entry::DecodeComplete, &selected[..count]),
        Err(ParserQueryRefusal::Entry)
    ));
    let family = families.for_entry(Entry::Seed).unwrap();
    let mut nominal = parser_canonical_nominal::PreparedParserNominal::prepare::<
        LanguageParserSessionSeedRequest,
    >(
        &family.borrow(),
        &["begin", "basis", "analysis_revision"],
        256,
        256,
        256,
    )
    .unwrap();
    assert_eq!(nominal.requested_bytes_bound(), 256);
    assert_eq!(nominal.retained_capacity_bytes(), 256);
    let oracle = LanguageAnalysisRevisionId::new("session/analysis/1".into())
        .unwrap()
        .encode()
        .unwrap();
    assert_eq!(nominal.leaf(b"session/analysis/1").unwrap(), oracle);
    let original = conduit_core::validate_canonical_structured_value(&oracle).unwrap();
    assert_eq!(
        nominal
            .compose(original.nominal_representation().unwrap())
            .unwrap(),
        oracle
    );
    assert!(matches!(
        nominal.compose(original),
        Err(parser_canonical_nominal::NominalRefusal::Type)
    ));
    family
        .borrow_mut()
        .decode::<LanguageAnalysisRevisionId>(nominal.leaf(b"session/analysis/1").unwrap())
        .unwrap();
    // Canonical framing does not waive the original nominal value contract.
    assert!(family
        .borrow_mut()
        .decode::<LanguageAnalysisRevisionId>(nominal.leaf(&[b'x'; 65]).unwrap())
        .is_err());
    assert!(matches!(
        parser_canonical_nominal::PreparedParserNominal::prepare::<LanguageParserSessionSeedRequest>(
            &family.borrow(),
            &["begin", "basis", "analysis_revision"],
            256,
            255,
            256
        ),
        Err(parser_canonical_nominal::NominalRefusal::Pressure)
    ));
    eprintln!("complete fixed query bank receipt={receipt:?}");
}
fn profile() -> Arc<PreparedCategoricalStep> {
    model_resource::categorical(
        include_bytes!("../training/ewt_joint_v2/ewt_joint.i16").to_vec(),
        parser_model_selection::pinned_v2_model_signature().unwrap(),
        1,
    )
}
fn query() -> LanguageParserV2ChoiceQuery {
    let provenance =
        LinguisticDerivationProvenance::deterministic_rule("test/custody".into(), "1".into())
            .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("test/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("test/revision".into()).unwrap(),
            "Hello, Travis!".into(),
        )
        .unwrap(),
        None,
        provenance,
        0,
        None,
    )
    .unwrap();
    let profile = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let lexical = lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
    let count = lexical.tape().tokens().len() as u64;
    assert_eq!(count, 4);
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("test/analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = LanguageParserState::new(
        basis,
        0,
        1,
        [5, 5, 5, 5, 4],
        relation.clone(),
        relation.clone(),
        relation.clone(),
        relation,
        [4, 4, 4, 4, 4],
        count,
        0,
    )
    .unwrap();
    LanguageParserV2ChoiceQuery::new(
        [0; 4],
        0,
        0,
        LanguageParserAvailableLexical::new(lexical.tape().clone(), count).unwrap(),
        state,
    )
    .unwrap()
}
fn family_limits() -> families::ProductionFamilyLimits {
    families::ProductionFamilyLimits {
        family: PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 1024 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
        },
        maximum_retained_bytes: 7 * 1024 * 1024 * 1024,
        maximum_preparation_peak_bytes: 7 * 1024 * 1024 * 1024,
        maximum_static_artifact_bytes: 1024 * 1024 * 1024,
        maximum_combined_bytes: 12 * 1024 * 1024 * 1024,
        other_reserved_bytes: 1024 * 1024 * 1024,
        concurrent_native_values: 2,
    }
}

struct KernelAdapter {
    execution: runtime::Execution,
    contract: parser_session_target_contract::ParserSessionTargetStorageContract,
    calls: Rc<core::cell::Cell<u32>>,
    cancels: Rc<core::cell::Cell<u32>>,
    corrupt: Rc<core::cell::Cell<bool>>,
}
impl parser_session_canonical_ingress::ParserCanonicalSourceExecutor for &mut KernelAdapter {
    type Error = &'static str;
    fn entry(&self) -> &str {
        &self.execution.entry
    }
    fn input_type_bytes(&self) -> &[u8] {
        &self.execution.input_type_bytes
    }
    fn output_type_bytes(&self) -> &[u8] {
        &self.execution.output_type_bytes
    }
    fn cancel(&mut self) {
        self.cancels.set(self.cancels.get() + 1);
        parser_session_canonical_ingress::ParserCanonicalSourceExecutor::cancel(
            &mut self.execution,
        );
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        let length = self.execution.transact_canonical(ordinal, input, output)?;
        if self.corrupt.get() && length > 0 {
            output[length - 1] ^= 1;
        }
        Ok(length)
    }
}
impl parser_session_fixed_ingress::ParserSessionExecutor for &mut KernelAdapter {
    fn original_plan(&self) -> &conduit_core::Plan {
        numeric_custody::ParserNumericExecutor::plan(&self.execution)
    }
}
impl numeric_custody::ParserNumericExecutor for &mut KernelAdapter {
    type Error = &'static str;
    fn plan(&self) -> &conduit_core::Plan {
        numeric_custody::ParserNumericExecutor::plan(&self.execution)
    }
    fn cancel(&mut self) {
        parser_session_canonical_ingress::ParserCanonicalSourceExecutor::cancel(self);
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, &'static str> {
        parser_session_canonical_ingress::ParserCanonicalSourceExecutor::transact(
            self, ordinal, input, output,
        )
    }
}
impl parser_session_target_contract::ParserSessionPreparedTarget for &mut KernelAdapter {
    fn checked_source(&self) -> &conduit_plot::CheckedSyntaxDocument {
        &self.execution.checked_source
    }
    fn expanded_source(&self) -> &conduit_plot::ExpandedAuthoringPlot {
        &self.execution.expanded_source
    }
    fn original_plan_owner(&self) -> Rc<conduit_core::Plan> {
        self.execution.original_plan.clone()
    }
    fn storage_contract(
        &self,
    ) -> parser_session_target_contract::ParserSessionTargetStorageContract {
        self.contract
    }
}
#[test]
fn actual_kernel_fixed_seed_preparation_execution_replay_and_late_output_refusal() {
    use parser_session_execution::{
        verification::PreparedSourceVerification, ParserSessionEntry as Entry,
    };
    use parser_session_fixed_ingress::{FixedRefusal, ParserFixedFrames};
    use parser_session_fixed_preparation::*;
    const GIB: usize = 1024 * 1024 * 1024;
    let families = families::PreparedProductionParserFamilies::prepare(family_limits()).unwrap();
    let receipt = families.receipt();
    let family = families.for_entry(Entry::Seed).unwrap();
    let profile = profile();
    let original = query();
    let lexical = LanguageParserJointLexical::new(
        original.lexical().tape().clone(),
        *original.lexical().token_count(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(
        original.state().basis().clone(),
        original.state().relation0().clone(),
        *original.state().token_count(),
    )
    .unwrap();
    let input = LanguageParserSessionSeedRequest::new(begin, 17, lexical)
        .unwrap()
        .encode()
        .unwrap();
    let calls = Rc::new(core::cell::Cell::new(0));
    let cancels = Rc::new(core::cell::Cell::new(0));
    let corrupt = Rc::new(core::cell::Cell::new(false));
    let mut adapter = KernelAdapter {
        execution: runtime::prepare_source_with_storage(
            profile.clone(),
            source(),
            Entry::Seed.name(),
            None,
        ),
        contract: parser_session_target_contract::ParserSessionTargetStorageContract::new(
            12 * GIB,
            GIB,
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        )
        .unwrap(),
        calls: calls.clone(),
        cancels: cancels.clone(),
        corrupt: corrupt.clone(),
    };
    let original_plan = adapter.execution.original_plan.clone();
    let other = receipt.retained_heap_bytes_bound
        + receipt.static_resources.canonical_bytes_bound
        + receipt.static_resources.descriptor_storage_bytes_bound
        + receipt.static_resources.source_canonical_bytes_bound
        + profile.storage_receipt().unwrap().retained_heap_bytes_bound
        + GIB // independently reserved complete seed refinement preparation
        + 4 * conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
    let limits = FixedPreparationLimits {
        verification: parser_session_execution::ParserSessionVerificationLimits {
            decoded_program_bytes: GIB,
            preparation_peak_bytes: GIB,
            retained_bytes: GIB,
        },
        maximum_metadata_temporary_bytes: GIB,
        maximum_plan_validation_temporary_bytes: 4 * GIB,
        maximum_endpoint_encoding_requested_bytes: 1024 * 1024,
        maximum_existing_target_bytes: 12 * GIB,
        other_existing_session_reserved_bytes: other,
        maximum_combined_bytes: 24 * GIB,
        maximum_invocations: 3,
    };
    let mut refused = limits;
    refused.maximum_combined_bytes = 0;
    assert!(matches!(
        prepare_fixed_target(&mut adapter, Entry::Seed, family.clone(), refused),
        Err(FixedRefusal::Pressure)
    ));
    assert_eq!(calls.get(), 0);
    let (mut port, prepared) =
        prepare_fixed_target(&mut adapter, Entry::Seed, family.clone(), limits).unwrap();
    eprintln!(
        "fixed Source preparation receipt={prepared:?}; input_bytes={}",
        input.len()
    );
    let too_small = ParserFixedFrames::prepare(
        input.len() - 1,
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    )
    .unwrap();
    assert!(matches!(
        port.execute(&input, too_small),
        Err(FixedRefusal::Pressure)
    ));
    assert_eq!(calls.get(), 0);
    let mut seed_refinement = parser_canonical_refinement::PreparedParserCanonicalRefinement::<
        LanguageParserSessionSeedProposal,
        LanguageParserJointRuntimeRawBeam,
    >::prepare(
        &family.borrow(),
        parser_canonical_refinement::ParserRefinementLimits {
            maximum_node_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_requested_bytes: GIB,
            maximum_preparation_requested_bytes: GIB,
        },
    )
    .unwrap();
    let seed_buffer = ParserFixedFrames::prepare(
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    )
    .unwrap()
    .into_candidate_buffer();
    let history = port
        .execute(
            &input,
            ParserFixedFrames::prepare(
                input.len(),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(Rc::ptr_eq(&history.original_plan, &original_plan));
    let seed_admission = parser_session_seed_admission::ParserSeedBeamAdmission::admit(
        0,
        &history,
        &mut seed_refinement,
        &mut family.borrow_mut(),
        seed_buffer,
    )
    .unwrap();
    assert_eq!(seed_admission.seed_execution, 0);
    seed_admission
        .readmit(&history, &mut seed_refinement, &mut family.borrow_mut())
        .unwrap();
    let (mut replay, _, _, _) =
        PreparedSourceVerification::prepare(Entry::Seed, limits.verification).unwrap();
    let conversion = family
        .borrow()
        .storage_receipt()
        .conversion_requested_bytes_bound;
    assert!(matches!(
        history.replay(
            &mut replay,
            &mut family.borrow_mut(),
            &original_plan,
            conversion - 1
        ),
        Err(FixedRefusal::Pressure)
    ));
    history
        .replay(
            &mut replay,
            &mut family.borrow_mut(),
            &original_plan,
            conversion,
        )
        .unwrap();
    let mut changed = history.output.clone();
    *changed.last_mut().unwrap() ^= 1;
    family
        .borrow_mut()
        .decode::<LanguageParserSessionSeedProposal>(&changed)
        .unwrap();
    corrupt.set(true);
    assert!(matches!(
        port.execute(
            &input,
            ParserFixedFrames::prepare(
                input.len(),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            )
            .unwrap()
        ),
        Err(FixedRefusal::DifferentOutput)
    ));
    assert_eq!(calls.get(), 2);
    assert!(cancels.get() > 0);
    assert!(matches!(
        port.execute(
            &input,
            ParserFixedFrames::prepare(
                input.len(),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            )
            .unwrap()
        ),
        Err(FixedRefusal::Cancelled)
    ));
    assert_eq!(calls.get(), 2);
}

#[test]
fn actual_kernel_complete_mixed_factory_preserves_original_plans_and_cancels_both_targets() {
    use conduit_ai::integer_categorical_step::{
        CategoricalCanonicalAdmissionLimits, PreparedCategoricalCanonicalAdmission,
    };
    use numeric_custody::{ParserNumericFrames, ParserNumericReadmissionBudget};
    use parser_session_canonical_ingress::PreparedParserExecutionFrames;
    use parser_session_execution::{
        verification::PreparedSourceVerification, ParserSessionEntry as Entry,
    };
    use parser_session_mixed_preparation::*;
    const GIB: usize = 1024 * 1024 * 1024;
    let model = profile();
    let lexical = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let selection = Arc::new(
        parser_model_selection::PreparedParserModelSelection::prepare(model.clone(), &lexical)
            .unwrap(),
    );
    let document=format!("{}\nplot production-model (\n features: LanguageParserV2ModelFeatures...| >> observed: LanguageParserV2ModelScores...|\n) {{\n features >> language-parser-v2-feature-indices() >> {}() >> language-parser-v2-score-observation() >> observed\n}}\n",source(),model.kind_identity(true));
    let numeric_execution =
        runtime::prepare_source_with_storage(model.clone(), document, "production-model", Some(2));
    let source_execution = runtime::prepare_checked_source_with_catalog(
        model.clone(),
        numeric_execution.source_document.clone(),
        numeric_execution.checked_source.clone(),
        numeric_execution.model_catalog.clone(),
        Entry::V2ModelFeatures.name(),
        Some(2),
    );
    assert!(Rc::ptr_eq(
        &numeric_execution.checked_source,
        &source_execution.checked_source
    ));
    assert!(Rc::ptr_eq(
        &numeric_execution.source_document,
        &source_execution.source_document
    ));
    assert!(Rc::ptr_eq(
        &numeric_execution.model_catalog,
        &source_execution.model_catalog
    ));
    let source_plan = source_execution.original_plan.clone();
    let numeric_plan = numeric_execution.original_plan.clone();
    let source_calls = Rc::new(std::cell::Cell::new(0));
    let numeric_calls = Rc::new(std::cell::Cell::new(0));
    let source_cancels = Rc::new(std::cell::Cell::new(0));
    let numeric_cancels = Rc::new(std::cell::Cell::new(0));
    let corrupt = Rc::new(std::cell::Cell::new(false));
    let contract = |input, output| {
        parser_session_target_contract::ParserSessionTargetStorageContract::new(
            12 * GIB,
            GIB,
            input,
            output,
        )
        .unwrap()
    };
    let mut source_adapter = KernelAdapter {
        execution: source_execution,
        contract: contract(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES, 4096),
        calls: source_calls.clone(),
        cancels: source_cancels.clone(),
        corrupt: Rc::new(std::cell::Cell::new(false)),
    };
    let mut numeric_adapter = KernelAdapter {
        execution: numeric_execution,
        contract: contract(4096, 4096),
        calls: numeric_calls.clone(),
        cancels: numeric_cancels.clone(),
        corrupt: corrupt.clone(),
    };
    let families = families::PreparedProductionParserFamilies::prepare(family_limits()).unwrap();
    let all = families.receipt();
    let family = families
        .for_values::<LanguageParserV2ChoiceQuery, LanguageParserV2ModelFeatures>()
        .unwrap();
    let verification = parser_session_execution::ParserSessionVerificationLimits {
        decoded_program_bytes: GIB,
        preparation_peak_bytes: GIB,
        retained_bytes: GIB,
    };
    let (mut feature_replay, _, _, feature_receipt) =
        PreparedSourceVerification::prepare(Entry::V2ModelFeatures, verification).unwrap();
    let (mut index_replay, _, _, index_receipt) =
        PreparedSourceVerification::prepare(Entry::V2FeatureIndices, verification).unwrap();
    let (mut scores_replay, _, _, scores_receipt) =
        PreparedSourceVerification::prepare(Entry::V2ScoreObservation, verification).unwrap();
    let canonical_limits = CategoricalCanonicalAdmissionLimits {
        maximum_preparation_peak_bytes: 4 * 1024 * 1024,
        maximum_retained_bytes: 4 * 1024 * 1024,
    };
    let mut numerical_replay =
        PreparedCategoricalCanonicalAdmission::prepare(model.clone(), canonical_limits).unwrap();
    let prepared_profile = parser_session_profile::PreparedParserSessionProfile::admit(
        parser_session_profile::ParserSessionProfile::PinnedFourSlotV2(selection.clone()),
        GIB,
    )
    .unwrap();
    let other = all.retained_heap_bytes_bound
        + all.static_resources.canonical_bytes_bound
        + all.static_resources.descriptor_storage_bytes_bound
        + all.static_resources.source_canonical_bytes_bound
        + prepared_profile.storage.combined_existing_input_bytes_bound
        + feature_receipt.retained_heap_bytes_bound
        + index_receipt.retained_heap_bytes_bound
        + scores_receipt.retained_heap_bytes_bound
        + numerical_replay.storage_receipt().retained_heap_bytes
        + 4 * conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
    let limits = MixedPreparationLimits {
        verification,
        canonical: canonical_limits,
        maximum_metadata_temporary_bytes: GIB,
        maximum_plan_validation_temporary_bytes: 16 * GIB,
        maximum_endpoint_encoding_requested_bytes: 1024 * 1024,
        other_existing_session_reserved_bytes: other,
        maximum_combined_bytes: 64 * GIB,
        maximum_invocations: 2,
    };
    let (mut owner, receipt) = prepare_mixed_targets(
        &mut source_adapter,
        &mut numeric_adapter,
        selection,
        family.clone(),
        limits,
    )
    .unwrap();
    assert_eq!(source_calls.get(), 0);
    assert_eq!(numeric_calls.get(), 0);
    assert_eq!(
        receipt.concurrently_live_native_bytes_bound,
        2 * family
            .borrow()
            .storage_receipt()
            .conversion_requested_bytes_bound
    );
    let mut frames = Vec::with_capacity(3);
    for _ in 0..3 {
        let source = PreparedParserExecutionFrames::prepare(
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            4096,
        )
        .unwrap();
        let mut output = Vec::with_capacity(4096);
        output.resize(4096, 0);
        frames.push((
            source,
            ParserNumericFrames {
                indices: Vec::with_capacity(4096),
                scores: Vec::with_capacity(4096),
                output,
            },
        ));
    }
    let input = query().encode().unwrap();
    let (source, numeric) = frames.pop().unwrap();
    let history = owner.execute(&input, source, numeric).unwrap();
    assert_eq!(source_calls.get(), 1);
    assert_eq!(numeric_calls.get(), 1);
    assert!(Rc::ptr_eq(&history.original_source_plan, &source_plan));
    assert!(Rc::ptr_eq(&history.numeric.original_plan, &numeric_plan));
    assert!(Arc::ptr_eq(&history.numeric.original_model, &model));
    let mut budget = ParserNumericReadmissionBudget {
        maximum_live_native_bytes: receipt.concurrently_live_native_bytes_bound,
    };
    let typed = history
        .replay_and_readmit(
            &mut feature_replay,
            &mut index_replay,
            &mut scores_replay,
            &mut numerical_replay,
            &mut family.borrow_mut(),
            &source_plan,
            &numeric_plan,
            &mut budget,
        )
        .unwrap();
    assert_eq!(typed.output.scores().len(), 76);
    drop(typed);
    let mut changed = history.numeric.output.clone();
    *changed.last_mut().unwrap() ^= 1;
    family
        .borrow_mut()
        .decode::<LanguageParserV2ModelScores>(&changed)
        .unwrap();
    corrupt.set(true);
    let (source, numeric) = frames.pop().unwrap();
    assert!(matches!(
        owner.execute(&input, source, numeric),
        Err(parser_session_mixed_custody::ParserMixedRefusal::Numeric(
            numeric_custody::ParserNumericRefusal::DifferentOutput
        ))
    ));
    assert_eq!(source_calls.get(), 2);
    assert_eq!(numeric_calls.get(), 2);
    assert!(source_cancels.get() > 0);
    assert!(numeric_cancels.get() > 0);
    let (source, numeric) = frames.pop().unwrap();
    assert!(matches!(
        owner.execute(&input, source, numeric),
        Err(parser_session_mixed_custody::ParserMixedRefusal::Cancelled)
    ));
    assert_eq!(source_calls.get(), 2);
    assert_eq!(numeric_calls.get(), 2);
    eprintln!("complete actual mixed factory receipt={receipt:?}");
}

#[path = "../src/parser_session_preparation.rs"]
mod parser_session_preparation;

#[test]
fn whole_session_preparation_refuses_aggregate_budget_before_metadata_or_ingress() {
    use parser_session_preparation::*;
    use parser_session_target_contract::*;
    struct Target(&'static str);
    impl parser_session_canonical_ingress::ParserCanonicalSourceExecutor for Target {
        type Error = ();
        fn cancel(&mut self) {}
        fn entry(&self) -> &str {
            self.0
        }
        fn input_type_bytes(&self) -> &[u8] {
            panic!("metadata must not be reached")
        }
        fn output_type_bytes(&self) -> &[u8] {
            panic!("metadata must not be reached")
        }
        fn transact(&mut self, _: u64, _: &[u8], _: &mut [u8]) -> Result<usize, Self::Error> {
            panic!("ingress must not be reached")
        }
    }
    impl parser_session_fixed_ingress::ParserSessionExecutor for Target {
        fn original_plan(&self) -> &conduit_core::Plan {
            panic!("Plan must not be reached")
        }
    }
    impl ParserSessionPreparedTarget for Target {
        fn checked_source(&self) -> &conduit_plot::CheckedSyntaxDocument {
            panic!("Source must not be reached")
        }
        fn expanded_source(&self) -> &conduit_plot::ExpandedAuthoringPlot {
            panic!("expansion must not be reached")
        }
        fn original_plan_owner(&self) -> Rc<conduit_core::Plan> {
            panic!("Plan owner must not be reached")
        }
        fn storage_contract(&self) -> ParserSessionTargetStorageContract {
            ParserSessionTargetStorageContract::new(1, 0, 1, 1).unwrap()
        }
    }
    const GIB: usize = 1024 * 1024 * 1024;
    let model = profile();
    let lexical = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let selection = Arc::new(
        parser_model_selection::PreparedParserModelSelection::prepare(model, &lexical).unwrap(),
    );
    let targets = parser_session_target_registry::REQUIRED
        .iter()
        .map(|entry| Target(entry.name()))
        .collect();
    let verification = parser_session_execution::ParserSessionVerificationLimits {
        decoded_program_bytes: GIB,
        preparation_peak_bytes: GIB,
        retained_bytes: GIB,
    };
    let fixed = parser_session_fixed_preparation::FixedPreparationLimits {
        verification,
        maximum_metadata_temporary_bytes: GIB,
        maximum_plan_validation_temporary_bytes: 16 * GIB,
        maximum_endpoint_encoding_requested_bytes: GIB,
        maximum_existing_target_bytes: GIB,
        other_existing_session_reserved_bytes: 0,
        maximum_combined_bytes: usize::MAX,
        maximum_invocations: 100,
    };
    let mixed = parser_session_mixed_preparation::MixedPreparationLimits {
        verification,
        canonical: conduit_ai::integer_categorical_step::CategoricalCanonicalAdmissionLimits {
            maximum_preparation_peak_bytes: GIB,
            maximum_retained_bytes: GIB,
        },
        maximum_metadata_temporary_bytes: GIB,
        maximum_plan_validation_temporary_bytes: 16 * GIB,
        maximum_endpoint_encoding_requested_bytes: GIB,
        other_existing_session_reserved_bytes: 0,
        maximum_combined_bytes: usize::MAX,
        maximum_invocations: 100,
    };
    let limits = ParserSessionPreparationLimits {
        family: family_limits(),
        fixed,
        mixed,
        queries: parser_session_queries::ParserQueryPreparationLimits {
            maximum_frame_bytes: 262144,
            maximum_preparation_requested_bytes: GIB,
            maximum_retained_requested_bytes: GIB,
        },
        maximum_existing_profile_bytes: GIB,
        maximum_historical_base_bytes: 16 * GIB,
        revision_and_driver_reserved_bytes: GIB,
        maximum_combined_bytes: 0,
    };
    let refused = prepare_session_owners(
        targets,
        Target("features"),
        NumericTargetBridge(Target("model")),
        parser_session_profile::ParserSessionProfile::PinnedFourSlotV2(selection.clone()),
        limits,
    );
    assert!(matches!(
        refused,
        Err(ParserSessionPreparationRefusal::Pressure)
    ));
    assert_eq!(selection.prepared_categorical().dimensions(), (413, 76, 25));
}

#[path = "../src/parser_session_rank.rs"]
mod parser_session_rank;

fn storage_only_fixture_plan() -> Rc<conduit_core::Plan> {
    use conduit_core::*;
    Rc::new(Plan {
        plan_id: "fixture/plan".into(),
        source_document_id: "fixture/source".into(),
        checked_plot_id: "fixture/checked".into(),
        expanded_plot_id: "fixture/expanded".into(),
        completion_policy: PlanCompletionPolicy::Live,
        realization_backs: vec![],
        activations: vec![],
        activation_preparations: vec![],
        fragments: vec![PlanFragment {
            plan_id: "fixture/plan".into(),
            fragment_id: "fixture/fragment".into(),
            source_document_id: "fixture/source".into(),
            checked_plot_id: "fixture/checked".into(),
            expanded_plot_id: "fixture/expanded".into(),
            completion_policy: PlanCompletionPolicy::Live,
            realization_backs: vec![],
            host_id: "fixture/host".into(),
            boot_id: "fixture/boot".into(),
            offer_generation: OfferGeneration(1),
            placements: vec![],
            execution_regions: vec![],
            execution_fusions: vec![],
            states: vec![],
            connections: vec![],
            fore_ports: vec![],
            shared_pools: vec![],
            startup_dependencies: vec![],
            startup_order: vec![],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![],
            expected_sign: vec![],
            sign_storage_budget: SignStorageBudget {
                item_capacity: 0,
                byte_capacity: 0,
            },
            plan_fragments: vec![],
        }],
    })
}
#[test]
fn revision_storage_reserves_ordered_events_and_refuses_locators_without_parent_material() {
    use parser_session_revision_custody::*;
    const GIB: usize = 1024 * 1024 * 1024;
    let model = profile();
    let profile = parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let selection = Arc::new(
        parser_model_selection::PreparedParserModelSelection::prepare(model, &profile).unwrap(),
    );
    let admitted = parser_session_profile::PreparedParserSessionProfile::admit(
        parser_session_profile::ParserSessionProfile::PinnedFourSlotV2(selection),
        GIB,
    )
    .unwrap();
    // This fixture exercises ownership/storage only. It is deliberately not a
    // Source/Plan authorization witness or public Session acceptance test.
    let base = parser_session_historical_base::ParserSessionHistoricalBase::prepare(
        admitted,
        vec![storage_only_fixture_plan()],
        0,
        GIB,
    )
    .unwrap();
    let query = query();
    let original_source = query.lexical().tape().source();
    let producer = lexical::prepare_lexical_tape(original_source, &profile, None).unwrap();
    let original = producer.tape().clone().encode().unwrap();
    let mut family = conduit_plot::rust_binding::PreparedNativeFamily::prepare(
        &[LanguageLexicalTape::PREPARED_DESCRIPTOR],
        family_limits().family,
    )
    .unwrap();
    let limits = RevisionStorageLimits {
        maximum_source_executions: 2,
        maximum_model_executions: 1,
        source_input_bytes: 262144,
        source_output_bytes: 262144,
        model_input_bytes: 4096,
        model_output_bytes: 4096,
        maximum_retained_chain_bytes: GIB,
        maximum_preparation_peak_bytes: GIB,
    };
    let mut book = ParserRevisionCustody::prepare(
        producer,
        &original,
        base.clone(),
        None,
        &mut family,
        limits,
    )
    .unwrap();
    assert_eq!(book.events.capacity(), 3);
    assert!(book.events.is_empty());
    book.validate_event_order().unwrap();
    let receipt = book.storage;
    let unique = Rc::get_mut(&mut book).unwrap();
    assert!(matches!(
        unique.retain_seed_admission(parser_session_seed_admission::ParserSeedBeamAdmission {
            seed_execution: 0,
            complete_beam: vec![]
        }),
        Err(RevisionStorageRefusal::OriginalTape)
    ));
    assert!(unique.events.is_empty());
    unique.events.push(ParserRevisionEvent::Model(0));
    assert!(matches!(
        unique.validate_event_order(),
        Err(RevisionStorageRefusal::OriginalTape)
    ));
    unique.events.clear();
    unique.validate_event_order().unwrap();
    let producer = lexical::prepare_lexical_tape(original_source, &profile, None).unwrap();
    assert!(matches!(
        ParserRevisionCustody::prepare(
            producer,
            &original,
            base,
            None,
            &mut family,
            RevisionStorageLimits {
                maximum_retained_chain_bytes: receipt.complete_retained_chain_bytes_bound - 1,
                ..limits
            }
        ),
        Err(RevisionStorageRefusal::Pressure)
    ));
}
