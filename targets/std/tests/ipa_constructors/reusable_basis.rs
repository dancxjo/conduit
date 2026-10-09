//! Three ordinary installed constructors consume one declared checked scope.
use super::*;

const SOURCE: &str =
    include_str!("../../../../semantics/speech/examples/ipa/reusable-basis.conduit");

#[derive(Default)]
struct Outputs(BTreeMap<String, ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::NormalClose {
            return Ok(());
        }
        assert_eq!(value.track, ConnectionTrack::Payload);
        assert!(
            self.0
                .insert(value.front_port_id.as_str().into(), value)
                .is_none(),
            "one delivery per Fore"
        );
        Ok(())
    }
}

#[test]
fn two_transcriptions_and_one_phoneme_reuse_original_scope_through_installed_kernel() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    validate_source(&syntax, &checked).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "reusable-basis", &profiles).unwrap();
    assert_eq!(authored.expanded.gears.len(), 3);
    for field in ["inventory", "basis", "phone-bindings", "phoneme-bindings"] {
        let values: Vec<_> = authored
            .expanded
            .gears
            .iter()
            .map(|gear| {
                gear.configuration
                    .iter()
                    .find(|entry| entry.key == field)
                    .unwrap()
            })
            .collect();
        assert!(values.iter().all(|entry| *entry == values[0]));
    }
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "reused-ipa-host".into(),
            boot_id: "reused-ipa-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal().with_native_speech(),
    );
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = ["first", "second", "unit"]
        .into_iter()
        .map(|name| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 262144,
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 262144,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert!(plan.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.host_calls.is_empty()
            && placement.resources.is_empty()
            && placement.authority.is_empty()));
    let mut outputs = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(outputs.0.len(), 3);
    for (node, fore, spelling, constructor) in [
        ("one", "first", "ˈt͡ʃaː", IpaConstructor::Phonemic),
        ("two", "second", "aːt͡ʃ", IpaConstructor::Phonemic),
        ("single", "unit", "t͡ʃ", IpaConstructor::Phoneme),
    ] {
        let gear = authored
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str().ends_with(&format!("/{node}")))
            .unwrap();
        let delivered = &outputs.0[fore];
        let expected = prepare_configuration(constructor, &gear.configuration).unwrap();
        assert_eq!(delivered.value_kind, *expected.value_kind());
        assert_eq!(delivered.bytes, expected.bytes());
        readmission::check(constructor, &gear.configuration, &delivered.bytes);
        if constructor == IpaConstructor::Phonemic {
            let decoded = SpeechPhonemicTranscription::decode(&delivered.bytes).unwrap();
            assert_eq!(decoded.original(), spelling);
            assert_eq!(decoded.encode().unwrap(), delivered.bytes);
        } else {
            let decoded = SpeechPhonemeNotation::decode(&delivered.bytes).unwrap();
            assert_eq!(decoded.definition().notation(), spelling);
            assert_eq!(decoded.encode().unwrap(), delivered.bytes);
        }
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel.playback.is_empty());
    assert!(kernel.kernel_sign.iter().all(|sign| !matches!(
        sign.kind,
        conduit_kernel::KernelEventKind::BackFailed
            | conduit_kernel::KernelEventKind::HostCallRequested
    )));
}
