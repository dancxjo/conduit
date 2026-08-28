use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, ObservationKind, TerminalDisposition};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;

const SPLIT_FORM: &str = "form portable_input {\n source: conduit-test/key-event-source\n split: input/key-tee\n keymap: input/keymap\n show: presentation/text\n chords: input/chords\n control: conduit-test/chord-sink\n source.key > split.key\n split.text-keys > keymap.key\n keymap.text > show.text\n split.chord-keys > chords.key\n chords.chord > control.chord\n}\n";

fn plan(source: &str) -> (super::StdHost, conduit_core::PlanFragment) {
    let host = host("portable-input-host");
    assert!(host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::KEYMAP_KIND));
    let form = parse(source, &installed_std::test_catalog()).expect("portable input Form checks");
    for kind in [
        conduit_semantic_catalog::KEY_EVENT_TEE_KIND,
        conduit_semantic_catalog::KEYMAP_KIND,
        conduit_semantic_catalog::CHORDS_KIND,
    ] {
        let Some(gear) = form.gears.iter().find(|gear| gear.kind_id.as_str() == kind) else {
            continue;
        };
        let offer = host
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == kind)
            .unwrap_or_else(|| panic!("missing offered {kind}"));
        assert_eq!(offer.checked_face(), gear.checked_face(), "{kind}");
    }
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("portable input placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("portable input Form plans with capacity-one Cords");
    (host, plan.fragments[0].clone())
}

#[test]
fn ordinary_form_splits_text_and_chords_through_the_production_kernel() {
    let (mut host, fragment) = plan(SPLIT_FORM);
    assert!(fragment
        .connections
        .iter()
        .all(|cord| cord.item_capacity == 1));
    let keymap = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::KEYMAP_KIND)
        .unwrap();
    assert_eq!(
        keymap.configuration[0].value,
        conduit_core::ConfigurationValue::Text(conduit_core::CONDUIT_INTL_LAYOUT.into())
    );
    let chords = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::CHORDS_KIND)
        .unwrap();
    assert_eq!(
        chords.configuration[0].value,
        conduit_core::ConfigurationValue::Text(conduit_core::CORE_CHORD_MAP.into())
    );

    let mut output = Vec::with_capacity(16_384);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(8),
    };
    let report = host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .expect("split input Form executes through the installed production scheduler");
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("\na\n"), "{output}");
    assert!(output.contains("\nb\n"), "{output}");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(
        kernel.post_play_start_allocations,
        0,
        "output_len={} output_capacity={}",
        output.len(),
        output.capacity()
    );
}

#[test]
fn keymap_text_flows_directly_into_text_upper_without_an_adapter() {
    let form = "form upper_input {\n source: conduit-test/key-event-source\n keymap: input/keymap\n upper: text/upper\n show: presentation/text\n source.key > keymap.key\n keymap.text > upper.text\n upper.text > show.text\n}\n";
    let (mut host, fragment) = plan(form);
    let mut output = Vec::with_capacity(16_384);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(8),
    };
    host.run_fragment_to(fragment, &mut output, &mut timer)
        .expect("keymap output is canonical text accepted by text/upper");
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("\nA\n"), "{output}");
    assert!(output.contains("\nB\n"), "{output}");
}

#[test]
fn unsupported_layout_and_cross_plane_wiring_refuse_before_play() {
    let unsupported = SPLIT_FORM.replace(
        "keymap: input/keymap",
        "keymap: input/keymap\n keymap.layout = \"host-locale\"",
    );
    assert!(parse(&unsupported, &installed_std::test_catalog()).is_err());

    let incompatible = SPLIT_FORM.replace("keymap.text > show.text", "chords.chord > show.text");
    assert!(parse(&incompatible, &installed_std::test_catalog()).is_err());
}
