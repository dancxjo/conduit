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
        + profile.storage_receipt().unwrap().retained_heap_bytes_bound;
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
