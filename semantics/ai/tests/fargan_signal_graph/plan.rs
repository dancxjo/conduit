use super::shared::*;
use conduit_ai::{fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*};
use conduit_core::*;
use std::collections::BTreeMap;
pub fn source() -> String {
    [
        include_str!("../../../speech/fargan_conditioning.conduit"),
        include_str!("../../../speech/fargan_signal.conduit"),
        include_str!("../../../speech/fargan_pitch_history.conduit"),
        include_str!("../../../speech/fargan_subframe.conduit"),
    ]
    .join("\n")
}
pub struct SourceSchema {
    pub ports: Vec<(String, StructuredInfoType, bool)>,
    pub resource_shapes: BTreeMap<String, Vec<u64>>,
}
impl SourceSchema {
    pub fn prepare() -> Self {
        Self::for_entry(&source(), "speech/fargan-subframe")
    }
    pub fn for_entry(source: &str, entry: &str) -> Self {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
        install_fixed_numeric_pair_catalogs(&mut startup, &mut profiles).unwrap();
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let mut types: BTreeMap<_, _> = fixed_numeric_types()
            .unwrap()
            .into_iter()
            .map(|t| (t.name, t.value_type))
            .collect();
        types.extend(
            checked
                .native_types
                .iter()
                .map(|t| (t.name.clone(), t.value_type.clone())),
        );
        let entry = checked.plots.iter().find(|p| p.name == entry).unwrap();
        let resource_shapes = entry
            .runtime_ports
            .iter()
            .filter_map(|p| {
                let name = &p.value_type.text;
                let shape = if let Some(shape) = name.strip_prefix("NumericF32MatrixRef") {
                    shape.split('x').map(|n| n.parse().unwrap()).collect()
                } else {
                    let shape = name.strip_prefix("NumericF32BiasRef")?;
                    vec![shape.parse().unwrap()]
                };
                Some((p.name.text.clone(), shape))
            })
            .collect();
        Self {
            resource_shapes,
            ports: entry
                .runtime_ports
                .iter()
                .map(|p| {
                    (
                        p.name.text.clone(),
                        types[&p.value_type.text].clone(),
                        p.direction == conduit_plot::syntax::RuntimePortDirection::Input,
                    )
                })
                .collect(),
        }
    }
    pub fn ty(&self, name: &str) -> &StructuredInfoType {
        &self.ports.iter().find(|p| p.0 == name).unwrap().1
    }
    pub fn zero_resources(&self) -> Resources {
        self.ports
            .iter()
            .filter(|(name, _, input)| {
                *input && name != "state" && name != "condition" && name != "period"
            })
            .map(|(name, ty, _)| {
                let dimensions = &self.resource_shapes[name];
                let mut values = vec![0.; dimensions.iter().product::<u64>() as usize];
                if name == "output_bias" {
                    values.fill(0.25);
                }
                (name.clone(), Resource::new(ty.clone(), dimensions, values))
            })
            .collect()
    }
}
pub fn prepare_plan(schema: &SourceSchema) -> Plan {
    prepare_entry_plan(schema, &source(), "speech/fargan-subframe")
}
pub fn prepare_entry_plan(schema: &SourceSchema, source: &str, entry: &str) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    install_fixed_numeric_pair_catalogs(&mut startup, &mut profiles).unwrap();
    let mut offers = vec![];
    let mut wrapper = format!("\nplot subframe-proof {{\n inner: {entry}\n");
    for (name, ty, input) in &schema.ports {
        let kind = fixture_kind(&format!("subframe-fixture/{name}"), ty, *input);
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
        wrapper.push_str(&format!(" {name}: subframe-fixture/{name}\n"));
        wrapper.push_str(&if *input {
            format!(" {name}.value >> inner.{name}\n")
        } else {
            format!(" inner.{name} >> {name}.value\n")
        });
    }
    wrapper.push_str("}\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&format!("{source}{wrapper}")),
        &startup,
    )
    .unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "subframe-proof", &profiles)
            .unwrap();
    assert!(authoring.input_bindings.is_empty() && authoring.output_bindings.is_empty());
    let plot = &authoring.expanded;
    for gear in &plot.gears {
        let selected = if gear.kind_id.as_str().starts_with("numeric/") {
            Some(
                conduit_std_host::fixed_numeric::fixed_numeric_offer(gear.kind_id.as_str())
                    .unwrap(),
            )
        } else if let [entry] = gear.configuration.as_slice() {
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("pure source program")
            };
            assert_eq!(entry.key, "program");
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            Some(conduitos::expression_host_call::offer(&program, PortTemporal::Value).unwrap())
        } else {
            None
        };
        if let Some(selected) = selected {
            if !offers
                .iter()
                .any(|o| o.capability_id == selected.capability_id)
            {
                offers.push(selected);
            }
        }
    }
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "subframe-proof".into(),
        boot_id: "subframe-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "synthetic-float-subframe@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_expanded_placements(plot, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical(
        plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
    )
    .unwrap()
}
