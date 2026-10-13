//! Checked source and allocation-free evaluation; no installed Body/input claim.
use conduit_core::{
    ConfigurationValue, HostAdvertisement, Plan, PortDirection, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, CanonicalBackCatalog, CheckedSyntaxDocument,
    ExpandedAuthoringPlot, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    prepare_checked_parsec_map_transition, prepare_checked_source_plan, CheckedSourcePlanInputs,
    ParsecMapCommand, ParsecMapState, ParsecMapTransitionRefusal, ParsecMapTransitionResult,
    PreparedCheckedSourcePlan, PreparedParsecMapTransition, PARSEC_MAP_FRAME_BYTES,
    PARSEC_MAP_TRANSITION_SOURCE,
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
    fn new(source: &str, name: &str) -> Self {
        let checked =
            check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
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
                        byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
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
                connection_byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
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
                    connection_byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
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
fn prepared() -> (PreparedParsecMapTransition, Fixture, Fixture) {
    let game = Fixture::new(PARSEC_MAP_TRANSITION_SOURCE, "parsec/state-step");
    let map = Fixture::new(PARSEC_MAP_TRANSITION_SOURCE, "parsec/map-transition");
    let evaluator = prepare_checked_parsec_map_transition(&game.custody(), &map.custody()).unwrap();
    (evaluator, game, map)
}
fn state(started: bool, keyboard: bool, revision: u64) -> Vec<u8> {
    ParsecMapState::new(keyboard, revision, started)
        .unwrap()
        .into_structured()
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn command(action: &str, keyboard: bool, expected: u64) -> Vec<u8> {
    ParsecMapCommand::new(action.into(), expected, keyboard)
        .unwrap()
        .into_structured()
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn decode(bytes: &[u8]) -> ParsecMapTransitionResult {
    ParsecMapTransitionResult::from_structured(
        StructuredInfoValue::from_canonical_bytes(bytes).unwrap(),
    )
    .unwrap()
}

#[test]
fn actual_game_step_then_source_remap_retains_state_and_both_plan_custodies() {
    let (mut evaluator, game, map) = prepared();
    let initial = state(false, false, 1);
    let start = command("game/start", false, 1);
    let result = evaluator.evaluate(&initial, &start).unwrap();
    assert_eq!(result.game_plan_id(), &game.plan.plan_id);
    assert_eq!(result.map_plan_id(), &map.plan.plan_id);
    assert_eq!(
        result.game_plot().checked_plot_id,
        game.expanded.expanded.checked_plot_id
    );
    assert_eq!(
        result.map_plot().expanded_plot_id,
        map.expanded.expanded.expanded_plot_id
    );
    assert!(*decode(result.result_bytes()).accepted());
    assert_eq!(result.state_bytes(), state(true, false, 2));
    let gameplay = result.state_bytes().to_vec();
    let shoot = command("game/shoot", false, 2);
    let result = evaluator.evaluate(&gameplay, &shoot).unwrap();
    assert!(*decode(result.result_bytes()).accepted());
    assert_eq!(result.state_bytes(), gameplay);
    let remap = command("game/remap", true, 2);
    let result = evaluator.evaluate(&gameplay, &remap).unwrap();
    assert!(*decode(result.result_bytes()).accepted());
    assert_eq!(result.state_bytes(), state(true, true, 3));
    let remapped = result.state_bytes().to_vec();
    for rejected in [
        command("game/start", true, 3),
        command("game/shoot", false, 2),
        command("game/invented", false, 3),
    ] {
        let result = evaluator.evaluate(&remapped, &rejected).unwrap();
        assert!(!*decode(result.result_bytes()).accepted());
        assert_eq!(result.state_bytes(), remapped);
    }
    let result = evaluator
        .evaluate(&remapped, &command("game/shoot", false, 3))
        .unwrap();
    assert!(*decode(result.result_bytes()).accepted());
    assert_eq!(
        result.state_bytes(),
        remapped,
        "Shoot cannot implicitly remap the chosen control"
    );
}

#[test]
fn revision_exhaustion_refuses_phase_and_control_change_without_partial_state() {
    let (mut evaluator, _, _) = prepared();
    let mut retained = state(false, false, 1);
    for revision in 1..17 {
        let input = command("game/remap", revision % 2 == 1, revision);
        let result = evaluator.evaluate(&retained, &input).unwrap();
        assert!(*decode(result.result_bytes()).accepted());
        assert_eq!(
            result.state_bytes(),
            state(false, revision % 2 == 1, revision + 1)
        );
        retained = result.state_bytes().to_vec();
    }
    assert_eq!(retained, state(false, false, 17));
    let exhausted = evaluator
        .evaluate(&retained, &command("game/remap", true, 17))
        .unwrap();
    assert!(!*decode(exhausted.result_bytes()).accepted());
    assert_eq!(exhausted.state_bytes(), retained);
    for (prior, input) in [
        (state(false, false, 17), command("game/start", false, 17)),
        (state(true, false, 17), command("game/remap", true, 17)),
    ] {
        let result = evaluator.evaluate(&prior, &input).unwrap();
        assert!(!*decode(result.result_bytes()).accepted());
        assert_eq!(result.state_bytes(), prior);
    }
    let prior = state(true, false, 17);
    let result = evaluator
        .evaluate(&prior, &command("game/shoot", true, 17))
        .unwrap();
    assert!(*decode(result.result_bytes()).accepted());
    assert_eq!(result.state_bytes(), prior);
}

#[test]
fn stale_and_forged_revision_or_context_cannot_change_state() {
    let (mut evaluator, _, _) = prepared();
    let prior = state(false, false, 2);
    let result = evaluator
        .evaluate(&prior, &command("game/start", false, 1))
        .unwrap();
    assert!(!*decode(result.result_bytes()).accepted());
    assert_eq!(result.state_bytes(), prior);
    let shape = ParsecMapState::semantic_type().unwrap();
    for revision in [0u64, 18] {
        let forged = StructuredInfoValue::record(
            shape.clone(),
            vec![
                StructuredFieldValue::new(
                    "keyboard",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID))
                            .unwrap(),
                        vec![0],
                    )
                    .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "mapping_revision",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),
                        revision.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "started",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID))
                            .unwrap(),
                        vec![0],
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert!(matches!(
            evaluator.evaluate(&forged, &command("game/start", false, 2)),
            Err(ParsecMapTransitionRefusal::State)
        ));
    }
    assert!(
        matches!(
            evaluator.evaluate(&prior, &prior),
            Err(ParsecMapTransitionRefusal::Command)
        ),
        "state supplied as command is not a context grant"
    );
}

#[test]
fn replaced_checked_program_refuses_even_with_reproduced_plan_and_same_shape() {
    let game = Fixture::new(PARSEC_MAP_TRANSITION_SOURCE, "parsec/state-step");
    let modified =
        PARSEC_MAP_TRANSITION_SOURCE.replace("mapping_revision < 17", "mapping_revision < 16");
    let map = Fixture::new(&modified, "parsec/map-transition");
    assert!(matches!(
        prepare_checked_parsec_map_transition(&game.custody(), &map.custody()),
        Err(ParsecMapTransitionRefusal::Source)
    ));
}

#[test]
fn prepared_evaluation_and_refusal_allocate_nothing_and_preserve_buffer_capacity() {
    let (mut evaluator, _, _) = prepared();
    let prior = state(false, false, 1);
    let accepted = command("game/start", false, 1);
    let stale = command("game/start", false, 2);
    let capacity = evaluator.prepared_buffer_bytes();
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|count| count.set(true));
    let good = evaluator.evaluate(&prior, &accepted).is_ok();
    let refused = evaluator.evaluate(&prior, &stale).is_ok();
    let invalid = evaluator.evaluate(&prior, &prior).is_err();
    COUNTING.with(|count| count.set(false));
    assert!(good && refused && invalid);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(evaluator.prepared_buffer_bytes(), capacity);
}
