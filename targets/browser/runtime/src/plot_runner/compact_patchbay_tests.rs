use super::*;

const MORSE: &str = r#"plot signal {
    message: text/literal("SOS")
    morse: text/morse
    light: presentation/indicator
    message >> morse >> light
}"#;

#[test]
fn projects_exact_typed_gears_ports_and_explicit_cords_without_host_facts() {
    let projection = project(MORSE, 7, false).unwrap();
    let external = project_plot(MORSE, 8).unwrap();
    assert_eq!(projection.schema, "conduit.tour/compact-patchbay@1");
    assert_eq!(
        external.schema,
        "conduit.patchbay/checked-plot-projection@1"
    );
    assert_eq!(external.checked_plot_id, projection.checked_plot_id);
    assert_eq!(external.gears, projection.gears);
    assert_eq!(external.cords, projection.cords);
    assert_eq!(projection.sequence, 7);
    assert_eq!(projection.gears.len(), 3);
    assert_eq!(projection.cords.len(), 2);
    assert!(projection.gears.iter().all(|gear| gear
        .inputs
        .iter()
        .chain(&gear.outputs)
        .all(|port| !port.info_kind.is_empty())));
    let encoded = serde_json::to_string(&projection).unwrap();
    for forbidden in ["host_id", "boot_id", "implementation_id", "plan_id"] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn projects_reusable_plot_with_unbound_front_port_for_authoring() {
    let source = r#"plot pulse-manifestation (
    >> tick: value/tick@1...
) {
    observe: time/pulse-observe(period-ms = 240)
    tick >> observe.tick
}"#;

    let projection = project(source, 8, false).unwrap();
    assert_eq!(projection.plot_name, "pulse-manifestation");
    assert_eq!(projection.gears.len(), 1);
    assert!(projection.cords.is_empty());
    assert!(projection.diagnostics.is_empty());
}

#[test]
fn sdk_projection_retains_exact_checked_front_constraints() {
    let source = r#"plot constrained (
    >> code: Text <= 8B in ["AB12", "CD34"] ~ /^[A-Z]{2}[0-9]{2}$/
) {
    upper: text/upper
    code >> upper
}"#;

    let projection = project_plot(source, 9).unwrap();
    assert_eq!(projection.front_inputs.len(), 1);
    let code = projection
        .front_inputs
        .iter()
        .find(|port| port.port_id == "code")
        .and_then(|port| port.value_contract.as_ref())
        .expect("browser SDK projection retains the exact code contract");
    assert_eq!(code.maximum_bytes, 8);
    assert_eq!(code.constraints.len(), 2);
    assert_eq!(code.validate(b"AB12"), Ok(()));
    assert_eq!(
        code.validate(b"EF56"),
        Err(conduit_core::ValueConstraintRefusal::Membership)
    );

    let encoded = serde_json::to_value(&projection).unwrap();
    assert_eq!(
        encoded["front_inputs"][0]["value_contract"]["value_kind"],
        "value/text"
    );
    assert_eq!(
        encoded["front_inputs"][0]["value_contract"]["constraints"][0]["CanonicalMembership"]
            ["members"][0],
        serde_json::json!([65, 66, 49, 50])
    );
    assert_eq!(
        encoded["front_inputs"][0]["value_contract"]["constraints"][0]["CanonicalMembership"]
            ["negated"],
        false
    );
}

#[test]
fn recursive_realization_preserves_the_front_and_carries_bounded_back_topology() {
    let direct = project(MORSE, 8, false).unwrap();
    let recursive = project(MORSE, 9, true).unwrap();
    assert_eq!(direct.gears, recursive.gears);
    assert_eq!(direct.cords, recursive.cords);
    assert_eq!(direct.realization_gears, direct.gears);
    assert_eq!(direct.realization_cords, direct.cords);
    assert_ne!(recursive.realization_gears, recursive.gears);
    assert_ne!(recursive.realization_cords, recursive.cords);
    assert!(recursive.realization_gears.len() <= MAXIMUM_BROWSER_PLOT_GEARS);
    assert!(recursive.realization_cords.len() <= MAXIMUM_BROWSER_PLOT_CORDS);
    assert_eq!(direct.checked_plot_id, recursive.checked_plot_id);
    assert_eq!(
        direct.visible_expanded_plot_id,
        recursive.visible_expanded_plot_id
    );
    assert_ne!(
        direct.realization_expanded_plot_id,
        recursive.realization_expanded_plot_id
    );
    assert!(direct.realization_backs.is_empty());
    assert!(!recursive.realization_backs.is_empty());
}

#[test]
fn invalid_cord_projects_the_draft_and_topology_bounds_still_refuse() {
    assert!(project("plot nope {", 1, false)
        .unwrap_err()
        .starts_with("parse checked-Plot Patchbay"));
    let wrong_type = r#"plot wrong {
    text: text/literal("x")
    light: presentation/indicator
    text >> light
}"#;
    let invalid = project(wrong_type, 2, false).unwrap();
    assert_eq!(invalid.realization, "invalid-source-proposal");
    assert!(invalid.visible_expanded_plot_id.is_empty());
    assert_eq!(invalid.gears.len(), 2);
    assert_eq!(invalid.cords.len(), 1);
    assert!(invalid.cords[0].invalid);
    assert_eq!(invalid.diagnostics[0].code, "CND-FRM-045");
    assert!(invalid.diagnostics[0].fix.contains("value/text"));
    assert!(invalid.diagnostics[0]
        .subjects
        .contains(&"wrong/light.receiving:pattern".to_owned()));

    let mut oversized = String::from("plot oversized {\n");
    for index in 0..=MAXIMUM_BROWSER_PLOT_GEARS {
        oversized.push_str(&format!("g{index}: text/literal(\"x\")\n"));
    }
    oversized.push('}');
    assert!(project(&oversized, 3, false)
        .unwrap_err()
        .contains("Gear bound exceeded"));
}
