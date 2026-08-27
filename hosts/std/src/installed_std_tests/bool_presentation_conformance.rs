use super::*;

const FORM: &str = "form bool_presentation {\n    source: test/timing-bool-source\n    show: presentation/bool\n    source > show\n}\n";

fn plan(host: &StdHost) -> conduit_core::Plan {
    let form =
        parse(FORM, &installed_std::test_catalog()).expect("Boolean presentation Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("Boolean presentation resolves");
    plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::BOOL_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("Boolean presentation plans")
}

#[test]
fn current_booleans_manifest_through_the_admitted_std_operation() {
    let mut host = host("bool-presentation-host");
    let plan = plan(&host);
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND)
        .expect("Boolean presenter is placed");
    assert_eq!(
        placement.implementation_id.as_str(),
        conduit_std_catalog::BOOL_PRESENTATION_STD_IMPLEMENTATION
    );
    assert_eq!(placement.host_operations.len(), 1);
    assert_eq!(placement.resources.len(), 1);

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(4),
    };
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("Boolean presentation executes through the production kernel");
    let output = String::from_utf8(output).unwrap();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("bool value="))
            .collect::<Vec<_>>(),
        ["bool value=false", "bool value=true", "bool value=false"]
    );
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn mutated_std_presenter_identity_refuses_before_play() {
    let host = host("mutated-bool-presentation-host");
    let mut plan = plan(&host);
    plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND)
        .unwrap()
        .artifact_id = conduit_core::ArtifactId::from("mutated/bool-presenter");
    let mut host = host;
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    assert!(host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .is_err());
    assert!(!String::from_utf8(output).unwrap().contains("bool value="));
    assert!(timer.waits.is_empty());
}
