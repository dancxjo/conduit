//! Checked component candidates only; no current map, installed input or renderer claim.
use conduit_core::{ConfigurationValue, HostAdvertisement, Plan, PortDirection};
use conduit_face::{
    checked_parsec_input_instruction, checked_parsec_input_policy,
    parsec_input_instruction_catalog, prepare_checked_parsec_input_instruction,
    prepare_checked_parsec_input_policy, prepare_checked_source_plan,
    ActionInputBindingProposalInfo, CheckedSourcePlanInputs, GeneratedActionAffordance,
    ParsecInputInstructionLook, ParsecInputInstructionRefusal, ParsecMapState,
    PreparedCheckedSourcePlan, PARSEC_INPUT_INSTRUCTION_FRAME_BYTES,
    PARSEC_INPUT_INSTRUCTION_SOURCE, PARSEC_INPUT_POLICY_FRAME_BYTES,
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
    fn new(checked: CheckedSyntaxDocument, name: &str) -> Self {
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
fn binding(started: bool, keyboard: bool, revision: u64, available: bool, label: &str) -> Vec<u8> {
    ActionInputBindingProposalInfo::new(
        available,
        "boot/current".into(),
        if keyboard {
            "keyboard/enter"
        } else {
            "control/fire"
        }
        .into(),
        label.into(),
        "face/current".into(),
        "host/current".into(),
        "plan/current-input".into(),
        revision,
        GeneratedActionAffordance::new(if started { "game/shoot" } else { "game/start" }.into(), 7)
            .unwrap(),
        "show/current".into(),
        1,
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn fixture() -> Fixture {
    Fixture::new(
        checked_parsec_input_instruction().unwrap(),
        "parsec/input-instruction",
    )
}

#[test]
fn exact_policy_candidates_form_three_source_wordings_without_allocation() {
    let fixture = fixture();
    let policy_fixture = Fixture::new(
        checked_parsec_input_policy().unwrap(),
        "parsec/input-policy",
    );
    let mut policy = prepare_checked_parsec_input_policy(&policy_fixture.custody()).unwrap();
    let mut producer = prepare_checked_parsec_input_instruction(&fixture.custody()).unwrap();
    let capacity = producer.prepared_buffer_bytes();
    for started in [false, true] {
        for keyboard in [false, true] {
            let state = state(started, keyboard, 2);
            let context = binding(started, keyboard, 2, true, "Candidate");
            let binding = policy
                .evaluate(&state, &context)
                .unwrap()
                .proposal_bytes()
                .to_vec();
            let label = if keyboard { "Enter" } else { "Space" };
            let verb = if started { "Shoot" } else { "Begin" };
            for (look, expected) in [
                (
                    ParsecInputInstructionLook::Arcade,
                    format!("Use {label} to {verb}."),
                ),
                (
                    ParsecInputInstructionLook::Linear,
                    format!("Input: {label} to {verb}."),
                ),
                (
                    ParsecInputInstructionLook::Spoken,
                    format!("To {verb}, use {label}."),
                ),
            ] {
                COUNTING.with(|enabled| enabled.set(true));
                ALLOCATIONS.with(|count| count.set(0));
                let candidate = producer.evaluate(&state, &binding, look, false).unwrap();
                assert_eq!(candidate.wording(), expected);
                assert_eq!(candidate.binding_bytes(), binding);
                assert!(!candidate.withdrawal());
                assert_eq!(candidate.producer_plan_id(), &fixture.plan.plan_id);
                assert_eq!(
                    candidate.producer_plot().checked_plot_id,
                    fixture.plan.checked_plot_id
                );
                COUNTING.with(|enabled| enabled.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                assert_eq!(producer.prepared_buffer_bytes(), capacity);
            }
        }
    }
}

#[test]
fn explicit_or_unavailable_withdrawal_has_no_activation_wording_and_preserves_binding() {
    let fixture = fixture();
    let mut producer = prepare_checked_parsec_input_instruction(&fixture.custody()).unwrap();
    let state = state(false, false, 1);
    for (available, explicit) in [(true, true), (false, false), (false, true)] {
        let binding = binding(false, false, 1, available, "Space");
        for look in [
            ParsecInputInstructionLook::Arcade,
            ParsecInputInstructionLook::Linear,
            ParsecInputInstructionLook::Spoken,
        ] {
            let candidate = producer.evaluate(&state, &binding, look, explicit).unwrap();
            assert_eq!(candidate.wording(), "Input unavailable.");
            assert!(candidate.withdrawal());
            assert_eq!(candidate.binding_bytes(), binding);
        }
    }
}

#[test]
fn unknown_control_label_is_forwarded_honestly_without_truncation() {
    let fixture = fixture();
    let mut producer = prepare_checked_parsec_input_instruction(&fixture.custody()).unwrap();
    let state = state(true, false, 2);
    let label = "é".repeat(32); // exact existing 64-byte label boundary
    let binding = binding(true, false, 2, true, &label);
    let candidate = producer
        .evaluate(&state, &binding, ParsecInputInstructionLook::Arcade, false)
        .unwrap();
    assert_eq!(candidate.wording(), format!("Use {label} to Shoot."));
    assert_eq!(candidate.binding_bytes(), binding);
    assert!(!candidate.wording().contains("Fire"));
    assert!(ParsecInputInstructionLook::parse("custom").is_err());
}

#[test]
fn malformed_overbound_cross_action_and_stale_revision_candidates_refuse_without_growth() {
    let fixture = fixture();
    let mut producer = prepare_checked_parsec_input_instruction(&fixture.custody()).unwrap();
    let state = state(true, false, 2);
    let current = binding(true, false, 2, true, "Space");
    let cross_action = binding(false, false, 2, true, "Space");
    let stale = binding(true, false, 1, true, "Space");
    let oversized = vec![0; PARSEC_INPUT_INSTRUCTION_FRAME_BYTES + 1];
    let invalid_state = forged_revision(&state, "mapping_revision", 18);
    let capacity = producer.prepared_buffer_bytes();
    COUNTING.with(|enabled| enabled.set(true));
    ALLOCATIONS.with(|count| count.set(0));
    for candidate in [&cross_action, &stale] {
        assert!(matches!(
            producer.evaluate(&state, candidate, ParsecInputInstructionLook::Arcade, false),
            Err(ParsecInputInstructionRefusal::Correlation)
        ));
    }
    assert!(matches!(
        producer.evaluate(
            &state,
            &oversized,
            ParsecInputInstructionLook::Arcade,
            false
        ),
        Err(ParsecInputInstructionRefusal::Binding)
    ));
    assert!(matches!(
        producer.evaluate(
            &oversized,
            &current,
            ParsecInputInstructionLook::Arcade,
            false
        ),
        Err(ParsecInputInstructionRefusal::State)
    ));
    assert!(matches!(
        producer.evaluate(
            &invalid_state,
            &current,
            ParsecInputInstructionLook::Arcade,
            false
        ),
        Err(ParsecInputInstructionRefusal::State)
    ));
    assert!(producer
        .evaluate(&state, &[0, 1], ParsecInputInstructionLook::Arcade, false)
        .is_err());
    COUNTING.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(producer.prepared_buffer_bytes(), capacity);
    assert!(producer
        .evaluate(&state, &current, ParsecInputInstructionLook::Arcade, false)
        .is_ok());
}

#[test]
fn reproduced_but_changed_instruction_source_is_not_the_checked_producer() {
    let source = PARSEC_INPUT_INSTRUCTION_SOURCE.replace("Use ", "Pretend ");
    let checked = check_syntax_document(
        &parse_syntax_document(&source),
        &parsec_input_instruction_catalog().unwrap(),
    )
    .unwrap();
    let changed = Fixture::new(checked, "parsec/input-instruction");
    assert!(matches!(
        prepare_checked_parsec_input_instruction(&changed.custody()),
        Err(ParsecInputInstructionRefusal::Source)
    ));
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
fn component_frame_inspection_preserves_original_binding_and_wording_without_allocation() {
    let fixture = fixture();
    let mut producer = prepare_checked_parsec_input_instruction(&fixture.custody()).unwrap();
    let mut inspection = conduit_face::PreparedParsecInstructionFrameInspection::prepare().unwrap();
    for look in [
        ParsecInputInstructionLook::Arcade,
        ParsecInputInstructionLook::Linear,
        ParsecInputInstructionLook::Spoken,
    ] {
        for withdrawal in [false, true] {
            let state = state(false, false, 2);
            let binding = binding(false, false, 2, true, "Space");
            let candidate = producer
                .evaluate(&state, &binding, look, withdrawal)
                .unwrap();
            COUNTING.with(|enabled| enabled.set(true));
            ALLOCATIONS.with(|count| count.set(0));
            let frame = inspection.inspect(candidate.canonical_bytes()).unwrap();
            assert_eq!(frame.canonical_bytes(), candidate.canonical_bytes());
            assert_eq!(frame.wording(), candidate.wording());
            assert_eq!(frame.withdrawal(), withdrawal);
            let original = conduit_core::validate_canonical_structured_value(&binding).unwrap();
            assert_eq!(frame.binding().type_bytes(), original.type_bytes());
            assert_eq!(frame.binding().value_node(), original.value_node());
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            COUNTING.with(|enabled| enabled.set(false));
        }
    }
    assert!(inspection.inspect(&[]).is_err());
    assert!(inspection.inspect(&[0; 4097]).is_err());
    assert!(inspection
        .inspect(&binding(false, false, 2, true, "Space"))
        .is_err());
}
