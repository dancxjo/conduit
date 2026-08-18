use super::{host, installed_std, BTreeMap, ConnectionBase, PlanningOptions, RecordingTimer};
use conduit_core::{ArtifactId, ObservationKind, TerminalDisposition, SCALAR_ENCODED_LEN};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options};

const FORM: &str = r#"form logic_decision {
 script: conduit-test/logic-script
 compare: logic/compare(operator = "eq")
 invert: logic/not
 choose: logic/select
 sink: conduit-test/logic-sink
 script.compare-left > compare.left
 script.compare-right > compare.right
 compare.out > invert.in
 invert.out > choose.selector
 script.when-false > choose.when-false
 script.when-true > choose.when-true
 choose.out > sink.in
}
"#;

fn plan() -> (super::StdHost, conduit_core::Plan) {
    let host = host("typed-logic-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("typed logic Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("logic placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("logic Form plans with capacity-one cords");
    (host, plan)
}

#[test]
fn compare_not_and_select_plan_and_execute_together_through_the_production_kernel() {
    let (mut host, plan) = plan();
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 5);
    assert_eq!(fragment.connections.len(), 7);
    assert!(fragment.connections.iter().all(|cord| {
        cord.item_capacity == 1 && cord.byte_capacity == SCALAR_ENCODED_LEN as u32
    }));
    for (kind, implementation) in [
        (
            conduit_std_catalog::LOGIC_COMPARE_KIND,
            conduit_std_catalog::LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
        ),
        (
            conduit_std_catalog::LOGIC_NOT_KIND,
            conduit_std_catalog::LOGIC_NOT_IMPLEMENTATION,
        ),
        (
            conduit_std_catalog::LOGIC_SELECT_KIND,
            conduit_std_catalog::LOGIC_SELECT_SCALAR_IMPLEMENTATION,
        ),
    ] {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .expect("logic placement exists");
        assert_eq!(placement.implementation_id.as_str(), implementation);
        assert!(placement
            .inputs
            .iter()
            .chain(placement.outputs.iter())
            .all(|port| port.value_kind.as_str() != conduit_std_catalog::GENERIC_VALUE_KIND));
    }

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("combined logic Form executes through the installed production kernel");
    assert!(timer.waits.is_empty());
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn unsupported_operator_and_incompatible_select_branch_fail_as_authored_forms() {
    let invalid_operator = FORM.replace("\"eq\"", "\"contains\"");
    assert!(parse(&invalid_operator, &installed_std::test_catalog()).is_err());

    let incompatible = r#"form incompatible_select {
 invert: logic/not
 choose: logic/select
 invert.out > choose.when-true
}
"#;
    assert!(parse(incompatible, &installed_std::test_catalog()).is_err());
}

#[test]
fn mutated_logic_implementation_fails_before_play() {
    let (mut host, plan) = plan();
    let mut fragment = plan.fragments[0].clone();
    fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::LOGIC_SELECT_KIND)
        .expect("select placement exists")
        .artifact_id = ArtifactId::from("mutated/logic-select");
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    assert!(host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());
}
