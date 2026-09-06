//! Independent metronome specimen through checked Form, planner, installation,
//! and production kernel. The recording timer proves requested waits, not wall time.
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use std::{collections::BTreeMap, time::Duration};

#[test]
fn reusable_pulse_form_plans_and_executes_outside_choir() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile).unwrap();
    let sink = crate::installed_std::pulse_observation_sink::offer();
    startup
        .insert(conduit_form::KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = format!("{}\nform metronome {{\n clock: time/tick(count = 3, period-ms = 320)\n pulse: pulse-observation(period-ms = 320, maximum-pulses = 3)\n clock.tick > pulse.tick\n result: conduit-test/pulse-sink\n pulse.observation > result.observation\n}}",include_str!("../../../../forms/pulse-observation/main.conduit"));
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "metronome", &profile).unwrap();
    let host = crate::StdHost::new_with_composition(
        crate::StdHostConfig {
            host_id: "pulse-proof".into(),
            boot_id: "pulse-proof/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        crate::StdHostComposition::minimal()
            .with_time()
            .with_pulse_observation(),
    );
    let mut advertisement = host.advertisement().clone();
    advertisement.capabilities.push(sink);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str()
            == conduit_std_offers::PULSE_OBSERVE_IMPLEMENTATION));
    struct Timer(Vec<Duration>);
    impl crate::TimerAdapter for Timer {
        fn wait(&mut self, duration: Duration) {
            self.0.push(duration);
        }
    }
    let mut timer = Timer(Vec::with_capacity(3));
    let mut output = Vec::with_capacity(16384);
    let report = crate::installed_std::run_fragment(
        crate::installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut 0,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .unwrap();
    assert_eq!(timer.0, [Duration::from_millis(320); 3]);
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}
