use super::*;
use conduit_core::ConfigurationValue;
use conduit_semantic_catalog::GearPalette;

const REFERENCE_PLOT: &str = r#"
plot handbook {
    greeting: text/literal("Hello")
    prefix: text/join("<&>")
    upper: text/upper
    label: text/join("Result: ")
    shown: presentation/text(maximum-values = 3)
    greeting >> prefix >> upper >> label >> shown
}
"#;

#[test]
fn generated_reference_agrees_with_patchbay_palette_and_checked_source() {
    let plot = tests::checked(REFERENCE_PLOT);
    let original = plot.clone();
    let palette = GearPalette::standard().unwrap();
    let svg = render_svg(&plot, &BTreeMap::new());
    let description = svg.split("<desc id=\"desc\">").nth(1).unwrap();
    let description = description.split("</desc>").next().unwrap();
    assert_eq!(plot.expanded.gears.len(), 5);
    assert_eq!(plot.expanded.connections.len(), 4);
    assert_eq!(description.matches("Gear ").count(), 5);

    for gear in &plot.expanded.gears {
        let entry = palette.find(&gear.kind_id).unwrap();
        assert_eq!(gear.kind_contract_revision, entry.kind_contract_revision);
        assert_eq!(gear.inputs, entry.inputs);
        assert_eq!(gear.outputs, entry.outputs);
        assert_eq!(gear.semantic_contract.laws, entry.semantic_laws);
        assert_eq!(
            gear.semantic_contract.configuration.len(),
            entry.configuration.len()
        );
        let heading = format!(
            "Gear {}; Kind {}; contract {}.",
            gear.gear_id.as_str(),
            entry.kind_id.as_str(),
            entry.kind_contract_revision.as_str()
        );
        let section = description.split(&heading).nth(1).unwrap();
        let section = section.split("Gear ").next().unwrap();
        for port in entry.inputs.iter().chain(&entry.outputs) {
            assert!(section.contains(&format!(
                "Port {:?} {}: {}; temporal {:?}; abnormal {}.",
                port.direction,
                escape(port.port_id.as_str()),
                escape(port.value_kind.as_str()),
                port.temporal,
                escape(&format!("{:?}", port.abnormal_kind))
            )));
        }
        for field in &entry.configuration {
            let checked = gear
                .semantic_contract
                .configuration
                .iter()
                .find(|checked| checked.key == field.key)
                .unwrap();
            assert_eq!(checked.default_value, field.default_value);
            assert_eq!(checked.rule, field.rule);
            assert!(section.contains(&format!(
                "Configuration {}: contract default {}; rule {}.",
                escape(&field.key),
                escape(&format!("{:?}", field.default_value)),
                escape(&format!("{:?}", field.rule))
            )));
        }
        for configured in &gear.configuration {
            assert!(section.contains(&format!(
                "Configured {} = {}.",
                escape(&configured.key),
                escape(&format!("{:?}", configured.value))
            )));
        }
    }
    for cord in &plot.expanded.connections {
        let source = plot
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id == cord.source_gear_id)
            .unwrap();
        let sink = plot
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id == cord.sink_gear_id)
            .unwrap();
        let output = source
            .outputs
            .iter()
            .find(|port| port.port_id == cord.source_port_id)
            .unwrap();
        let input = sink
            .inputs
            .iter()
            .find(|port| port.port_id == cord.sink_port_id)
            .unwrap();
        conduit_plot::validate_connection_contract(output, input, cord.track).unwrap();
        assert_eq!(cord.value_kind, output.value_kind);
    }
    assert!(description.contains("Configured maximum-values = U64(3)."));
    assert!(description.contains("Configured prefix = Text(&quot;&lt;&amp;&gt;&quot;)."));
    assert!(!description.contains("<&>"));
    assert!(description.contains("not current Host availability or a Plan"));
    assert_eq!(plot, original);
}

#[test]
fn generated_reference_keeps_checked_contract_not_a_same_named_catalog_revision() {
    let mut plot = tests::checked(REFERENCE_PLOT);
    let gear = &mut plot.expanded.gears[0];
    gear.kind_contract_revision = conduit_core::KindIdentity::from("test/exact-retained-revision");
    gear.semantic_contract.configuration[0].default_value =
        ConfigurationValue::Text("retained <default>".into());
    let svg = render_svg(&plot, &BTreeMap::new());
    assert!(svg.contains("contract test/exact-retained-revision."));
    assert!(svg.contains("contract default Text(&quot;retained &lt;default&gt;&quot;)"));
}
