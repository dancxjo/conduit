use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, ThreadTimer};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../forms/desk-telegraph/main.conduit");

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_record_temporal_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_ordered_record_queue_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &startup).expect("Desk Telegraph checks");
    expand_canonical_form(&checked, "desk_telegraph", &profile)
        .expect("Desk Telegraph recursively expands")
}

fn plan(host: &StdHost, expanded: &conduit_form::ExpandedCanonicalForm) -> conduit_core::Plan {
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(expanded, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        expanded,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}

#[test]
fn canonical_desk_telegraph_frames_and_recovers_text_through_one_kernel_play() {
    let expanded = expanded();
    let kinds: Vec<_> = expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect();
    for kind in [
        "text/literal",
        conduit_net::TEXT_TO_TYPED_RECORD_KIND,
        conduit_net::TYPED_RECORD_FRAME_KIND,
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
        conduit_net::TYPED_RECORD_TO_TEXT_KIND,
        conduit_net::RECORD_SINGLETON_STREAM_KIND,
        conduit_net::ORDERED_RECORD_QUEUE_KIND,
        conduit_net::RECORD_EXACTLY_ONE_KIND,
        "presentation/text",
    ] {
        assert!(kinds.contains(&kind), "missing expanded {kind}");
    }

    let mut host = StdHost::new();
    let plan = plan(&host, &expanded);
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 9);

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("Desk Telegraph executes through the production kernel");
    assert!(String::from_utf8_lossy(&output).contains("CALLING\n"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel execution evidence exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn missing_or_stale_codec_realization_refuses_before_presentation() {
    let expanded = expanded();
    let mut host = StdHost::new();
    let mut plan = plan(&host, &expanded);
    let frame = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_net::TYPED_RECORD_FRAME_KIND)
        .unwrap();
    frame.artifact_id = "wrong/typed-record-codec@1".into();

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    assert!(host
        .run_fragment_to(plan.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(!String::from_utf8_lossy(&output).contains("CALLING"));
}
