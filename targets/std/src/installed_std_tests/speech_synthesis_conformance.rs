use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use std::collections::BTreeMap;

#[test]
fn unchanged_speech_form_streams_several_blocks_through_ordinary_plan_and_play() {
    let mut catalog = installed_std::test_catalog();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut catalog)
        .expect("speech synthesis catalog is exact");
    let form = conduit_form::parse(
        "form bounded_speech {\n synthesize: speech/synthesize(maximum-output-bytes = 32768)\n sink: conduit-test/speech-pcm-sink\n \"Rosehip House is ready.\" > synthesize.text\n synthesize.audio > sink.audio\n}\n",
        &catalog,
    )
    .expect("provider-neutral speech Form parses");
    let mut host = host("deterministic-speech-host");
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements)
        .expect("initialized deterministic speech offer is selected");
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::MAXIMUM_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("provider-neutral speech Form plans through the std offer");
    let synthesis = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_tongues::SPEECH_SYNTHESIZE_KIND)
        .expect("speech placement exists");
    assert_eq!(
        synthesis.execution_profile_id.as_str(),
        conduit_std_offers::DETERMINISTIC_SPEECH_PROFILE
    );
    assert_eq!(synthesis.configuration[0].key, "maximum-output-bytes");

    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(1_024),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("three fake provider blocks stream through the production kernel");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
