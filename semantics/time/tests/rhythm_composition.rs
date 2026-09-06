//! Recursive Form proof, separate from Host execution and live synchronization.
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};

const OBSERVATION: &str = include_str!("../../../forms/pulse-observation/main.conduit");
const SYNCHRONIZATION: &str = include_str!("../../../forms/phase-synchronization/main.conduit");
const CONSUMER: &str = "form sampling-heartbeat {
    ticks: proof/ticks
    state: proof/state
    pulse: pulse-observation
    follower: phase-synchronization
    result: proof/result
    ticks.tick > pulse.tick
    pulse.observation > follower.peer
    state.state > follower.local
    follower.updated > result.state
}";

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile).unwrap();
    for (kind, name, value, direction) in [
        (
            "proof/ticks",
            "tick",
            conduit_time::TICK_VALUE_KIND,
            PortDirection::Output,
        ),
        (
            "proof/state",
            "state",
            conduit_time::RHYTHM_STATE_VALUE_KIND,
            PortDirection::Output,
        ),
        (
            "proof/result",
            "state",
            conduit_time::RHYTHM_STATE_VALUE_KIND,
            PortDirection::Input,
        ),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        let port = PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(value),
            direction,
            temporal: PortTemporal::Flow { closes: true },
        };
        profile
            .insert(KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from("proof/rhythm@1"),
                inputs: if direction == PortDirection::Input {
                    vec![port.clone()]
                } else {
                    vec![]
                },
                outputs: if direction == PortDirection::Output {
                    vec![port]
                } else {
                    vec![]
                },
                configuration: vec![],
            })
            .unwrap();
    }
    (startup, profile)
}

#[test]
fn canonical_rhythm_forms_compose_as_gears_in_an_independent_closed_form() {
    let (startup, profile) = catalogs();
    let source = [OBSERVATION, SYNCHRONIZATION, CONSUMER].join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "sampling-heartbeat", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 5);
    assert_eq!(expanded.connections.len(), 4);
    for (canonical, name, primitive) in [
        (
            OBSERVATION,
            "pulse-observation",
            conduit_time::PULSE_OBSERVE_KIND,
        ),
        (
            SYNCHRONIZATION,
            "phase-synchronization",
            conduit_time::PHASE_SYNCHRONIZE_KIND,
        ),
    ] {
        let independent =
            check_syntax_document(&parse_syntax_document(canonical), &startup).unwrap();
        let nested = checked.forms.iter().find(|form| form.name == name).unwrap();
        assert_eq!(independent.forms[0].checked_form_id, nested.checked_form_id);
        let gear = expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == primitive)
            .unwrap();
        let provenance = expanded
            .provenance
            .iter()
            .find(|entry| entry.gear_id == gear.gear_id.as_str())
            .unwrap();
        assert_eq!(provenance.source_form, name);
        assert!(provenance.form_path.len() > 1);
    }
}

#[test]
fn missing_reusable_definition_and_incompatible_nested_port_refuse() {
    let (startup, profile) = catalogs();
    let missing = [SYNCHRONIZATION, CONSUMER].join("\n");
    assert_eq!(
        check_syntax_document(&parse_syntax_document(&missing), &startup)
            .unwrap_err()
            .code,
        "CND-FRM-028"
    );
    let incompatible = CONSUMER.replace("ticks.tick > pulse.tick", "state.state > pulse.tick");
    let source = [OBSERVATION, SYNCHRONIZATION, &incompatible].join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "sampling-heartbeat", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}

#[test]
fn heartbeat_face_has_runtime_inputs_and_outputs_without_startup_parameters() {
    let (startup, profile) = catalogs();
    let source = include_str!("../../../forms/heartbeat-phase-follower/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    assert!(checked.forms[0].startup_parameters.is_empty());
    let authoring = conduit_form::expand_canonical_form_for_authoring(
        &checked,
        "heartbeat-phase-follower",
        &profile,
    )
    .unwrap();
    let inputs: Vec<_> = authoring
        .input_bindings
        .iter()
        .map(|binding| binding.face_port_id.as_str())
        .collect();
    let outputs: Vec<_> = authoring
        .output_bindings
        .iter()
        .map(|binding| binding.face_port_id.as_str())
        .collect();
    assert_eq!(inputs, ["local", "peer", "tick"]);
    assert_eq!(outputs, ["heartbeat", "updated"]);
    assert_eq!(
        expand_canonical_form(&checked, "heartbeat-phase-follower", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-033"
    );
}
