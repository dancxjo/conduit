//! Explicit temporal contracts for the same finite authored feature recipe.
use super::*;
pub(super) fn feature_source(flow: bool) -> String {
    let prefix = "type FarganPeriod = U16 in 32..=255\n";
    if flow {
        format!(
            "{prefix}{}\n{}",
            include_str!("../../../speech/fargan_feature_policy_flow.conduit"),
            include_str!("../../../speech/fargan_feature_correlation_flow.conduit")
        )
    } else {
        format!(
            "{prefix}{}\n{}",
            include_str!("../../../speech/fargan_feature_policy.conduit"),
            include_str!("../../../speech/fargan_feature_correlation.conduit")
        )
    }
}
#[test]
fn closing_feature_recipe_preserves_every_finite_expression_program_with_explicit_flow_owners() {
    let (startup, profiles) = catalogs(true);
    let value =
        check_syntax_document(&parse_syntax_document(&feature_source(false)), &startup).unwrap();
    let flow =
        check_syntax_document(&parse_syntax_document(&feature_source(true)), &startup).unwrap();
    let mut programs = 0;
    for plot in &value.plots {
        let value = expand_canonical_plot_for_authoring(&value, &plot.name, &profiles).unwrap();
        let name = plot.name.replace("speech/fargan-", "speech/flow-fargan-");
        let flow = expand_canonical_plot_for_authoring(&flow, &name, &profiles).unwrap();
        let expression = |graph: &ExpandedAuthoringPlot| -> Option<PortableExpressionProgram> {
            if graph.expanded.gears.len() != 1 {
                return None;
            }
            let ConfigurationValue::Text(encoded) =
                &graph.expanded.gears[0].configuration.first()?.value
            else {
                return None;
            };
            PortableExpressionProgram::from_canonical_hex(encoded).ok()
        };
        if let Some(expected) = expression(&value) {
            assert_eq!(expression(&flow).unwrap(), expected, "{name}");
            programs += 1;
        }
    }
    assert!(programs >= 20, "all finite numerical Source laws compared");
    let graph =
        expand_canonical_plot_for_authoring(&flow, "speech/flow-fargan-feature-frame", &profiles)
            .unwrap();
    assert!(!graph.expanded.gears.is_empty());
    let resources = graph
        .front
        .inputs()
        .iter()
        .filter(|port| port.temporal == PortTemporal::Value)
        .map(|port| port.value_kind.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(resources.len(), 3);
    for gear in &graph.expanded.gears {
        for port in gear.inputs.iter().chain(&gear.outputs) {
            if port.temporal == PortTemporal::Value {
                assert!(
                    resources.contains(&port.value_kind),
                    "nonresource Value port in closing feature graph: {gear:?}"
                );
            } else {
                assert_eq!(
                    port.temporal,
                    PortTemporal::Flow { closes: true },
                    "{gear:?}"
                );
            }
        }
    }
    eprintln!("explicit closing feature frame: {}nodes/{}cords; {programs} finite expression programs unchanged",graph.expanded.gears.len(),graph.expanded.connections.len());
}
