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
        + 2 * conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;
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
