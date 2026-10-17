//! Prepared policy source custody only; no current map or installed input claim.
use conduit_core::{ConfigurationValue, HostAdvertisement, Plan, PortDirection};
use conduit_face::{
    checked_parsec_input_policy, parsec_input_policy_catalog, prepare_checked_parsec_input_policy,
    prepare_checked_source_plan, ActionInputBindingProposalInfo, CheckedSourcePlanInputs,
    GeneratedActionAffordance, ParsecInputPolicyRefusal, ParsecMapState, PreparedCheckedSourcePlan,
    PARSEC_INPUT_POLICY_FRAME_BYTES, PARSEC_INPUT_POLICY_SOURCE,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, CanonicalBackCatalog, CheckedSyntaxDocument,
    ExpandedAuthoringPlot, PortableExpressionProgram, ProfileCatalog,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::BTreeMap,
};

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
struct CountAllocations;
unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNTING.try_with(|enabled| {
            if enabled.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = COUNTING.try_with(|enabled| {
            if enabled.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;

struct Fixture {
    checked: CheckedSyntaxDocument,
    expanded: ExpandedAuthoringPlot,
    host: [HostAdvertisement; 1],
    placements: PlacementChoices,
    boundaries: BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
    plan: Plan,
}
impl Fixture {
    fn new(checked: CheckedSyntaxDocument) -> Self {
        let name = "parsec/input-policy";
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{name} expansion failed: {error:?}"));
        let gear = &expanded.expanded.gears[0];
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("exact program");
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let host = [HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: "fixture/parsec-map-host".into(),
            boot_id: "fixture/parsec-map-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            profile: "fixture/parsec-source".into(),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![conduit_std_offers::pure_expression_std_offer(
                &program,
                gear.inputs[0].temporal,
            )
            .unwrap()],
        }];
        let placements =
            conduit_planner::default_expanded_placements(&expanded.expanded, &host).unwrap();
        let boundaries = expanded
            .input_bindings
            .iter()
            .map(|binding| (PortDirection::Input, binding))
            .chain(
                expanded
                    .output_bindings
                    .iter()
                    .map(|binding| (PortDirection::Output, binding)),
            )
            .map(|(direction, binding)| {
                (
                    ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: PARSEC_INPUT_POLICY_FRAME_BYTES as u32,
                    },
                )
            })
            .collect();
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &expanded,
            &host,
            &placements,
            &[],
            PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: PARSEC_INPUT_POLICY_FRAME_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        Self {
            checked,
            expanded,
            host,
            placements,
            boundaries,
            plan,
        }
    }
    fn custody(&self) -> PreparedCheckedSourcePlan<'_> {
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        prepare_checked_source_plan(
            &self.checked,
            &self.expanded,
            &ProfileCatalog::new(),
            &CanonicalBackCatalog::new(),
            &self.plan,
            CheckedSourcePlanInputs {
                hosts: &self.host,
                placements: &self.placements,
                bases: &[],
                options: PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &lines,
                    connection_item_capacity: 1,
                    connection_byte_capacity: PARSEC_INPUT_POLICY_FRAME_BYTES as u32,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                boundary_limits: &self.boundaries,
            },
        )
        .unwrap()
    }
}
fn state(started: bool, keyboard: bool, revision: u64) -> Vec<u8> {
    ParsecMapState::new(keyboard, revision, started)
        .unwrap()
        .encode()
        .unwrap()
}
fn context(revision: u64) -> Vec<u8> {
    ActionInputBindingProposalInfo::new(
        true,
        "boot/current".into(),
        "control/forged".into(),
        "Forged label".into(),
        "face/current".into(),
        "host/current".into(),
        "plan/current-input".into(),
        revision,
        GeneratedActionAffordance::new("game/forged".into(), 7).unwrap(),
        "show/current".into(),
        1,
    )
    .unwrap()
    .encode()
    .unwrap()
}

#[test]
fn prepared_policy_evaluates_exact_phase_remap_and_custody_without_allocation() {
    let fixture = Fixture::new(checked_parsec_input_policy().unwrap());
    let mut evaluator = prepare_checked_parsec_input_policy(&fixture.custody()).unwrap();
    let capacity = evaluator.prepared_buffer_bytes();
    for started in [false, true] {
        for keyboard in [false, true] {
            let state = state(started, keyboard, 2);
            let context = context(2);
            let mut capture = Vec::with_capacity(PARSEC_INPUT_POLICY_FRAME_BYTES);
            COUNTING.with(|enabled| enabled.set(true));
            ALLOCATIONS.with(|count| count.set(0));
            let proposal = evaluator.evaluate(&state, &context).unwrap();
            assert_eq!(proposal.producer_plan_id(), &fixture.plan.plan_id);
            assert_eq!(
                proposal.producer_plot().checked_plot_id,
                fixture.plan.checked_plot_id
            );
            capture.extend_from_slice(proposal.proposal_bytes());
            COUNTING.with(|enabled| enabled.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            assert_eq!(evaluator.prepared_buffer_bytes(), capacity);
            let proposal = ActionInputBindingProposalInfo::decode(&capture).unwrap();
            assert_eq!(
                proposal.offered_action().action_identity(),
                if started { "game/shoot" } else { "game/start" }
            );
            assert_eq!(
                proposal.control_identity(),
                if keyboard {
                    "keyboard/enter"
                } else {
                    "control/fire"
                }
            );
            assert_eq!(
                proposal.control_label(),
                if keyboard { "Enter" } else { "Space" }
            );
            assert_eq!(*proposal.mapping_revision(), 2);
            assert_eq!(proposal.host_id(), "host/current");
            assert_eq!(proposal.boot_id(), "boot/current");
            assert_eq!(proposal.input_plan_id(), "plan/current-input");
            assert_eq!(proposal.face_identity(), "face/current");
            assert_eq!(proposal.show_identity(), "show/current");
            assert_eq!(*proposal.offered_action().source_face_revision(), 7);
        }
    }
}

#[test]
fn malformed_frames_refuse_with_stable_prepared_storage() {
    let fixture = Fixture::new(checked_parsec_input_policy().unwrap());
    let mut evaluator = prepare_checked_parsec_input_policy(&fixture.custody()).unwrap();
    let capacity = evaluator.prepared_buffer_bytes();
    let state = state(false, false, 1);
    let context = context(1);
    let oversize = vec![0; PARSEC_INPUT_POLICY_FRAME_BYTES + 1];
    COUNTING.with(|enabled| enabled.set(true));
    ALLOCATIONS.with(|count| count.set(0));
    assert!(matches!(
        evaluator.evaluate(&context, &context),
        Err(ParsecInputPolicyRefusal::State)
    ));
    assert!(matches!(
        evaluator.evaluate(&state, &state),
        Err(ParsecInputPolicyRefusal::Context)
    ));
    assert!(matches!(
        evaluator.evaluate(&oversize, &context),
        Err(ParsecInputPolicyRefusal::State)
    ));
    assert!(matches!(
        evaluator.evaluate(&state, &oversize),
        Err(ParsecInputPolicyRefusal::Context)
    ));
    COUNTING.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(evaluator.prepared_buffer_bytes(), capacity);
    assert!(evaluator.evaluate(&state, &context).is_ok());
}

fn forged_revision(canonical: &[u8], field_name: &str, revision: u64) -> Vec<u8> {
    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(canonical).unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("native owner record");
    };
    let fields = fields
        .iter()
        .map(|field| {
            if field.name() != field_name {
                return field.clone();
            }
            conduit_core::StructuredFieldValue::new(
                field.name(),
                conduit_core::StructuredInfoValue::leaf(
                    field.value().value_type().clone(),
                    revision.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    conduit_core::StructuredInfoValue::record(value.value_type().clone(), fields)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

#[test]
fn canonical_frames_with_forged_owner_refinements_are_refused() {
    let fixture = Fixture::new(checked_parsec_input_policy().unwrap());
    let mut evaluator = prepare_checked_parsec_input_policy(&fixture.custody()).unwrap();
    let state = state(false, false, 1);
    let context = context(1);
    for revision in [0, 18] {
        let forged = forged_revision(&state, "mapping_revision", revision);
        assert!(matches!(
            evaluator.evaluate(&forged, &context),
            Err(ParsecInputPolicyRefusal::State)
        ));
    }
    let forged = forged_revision(&context, "mapping_revision", 0);
    assert!(matches!(
        evaluator.evaluate(&state, &forged),
        Err(ParsecInputPolicyRefusal::Context)
    ));
}

#[test]
fn different_checked_policy_is_refused_even_with_its_reproduced_plan() {
    let startup = parsec_input_policy_catalog().unwrap();
    let changed = PARSEC_INPUT_POLICY_SOURCE.replace("\"Enter\"", "\"Changed\"");
    let checked = check_syntax_document(&parse_syntax_document(&changed), &startup).unwrap();
    let fixture = Fixture::new(checked);
    assert!(matches!(
        prepare_checked_parsec_input_policy(&fixture.custody()),
        Err(ParsecInputPolicyRefusal::Source)
    ));
}

#[test]
fn source_proposal_does_not_establish_context_revision_authority() {
    let fixture = Fixture::new(checked_parsec_input_policy().unwrap());
    let mut evaluator = prepare_checked_parsec_input_policy(&fixture.custody()).unwrap();
    let state = state(true, true, 3);
    let context = context(2);
    let proposal = evaluator.evaluate(&state, &context).unwrap();
    let proposal = ActionInputBindingProposalInfo::decode(proposal.proposal_bytes()).unwrap();
    // Existing authored policy forwards context revision. The future private
    // owner must refuse this mismatch before treating the proposal as current.
    assert_eq!(*proposal.mapping_revision(), 2);
    assert_eq!(proposal.offered_action().action_identity(), "game/shoot");
}
