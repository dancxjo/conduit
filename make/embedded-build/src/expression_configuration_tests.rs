//! Fixed image admission of the actual DHT11 Source program, not arbitrary text.
use crate::{
    generate_embedded_plan, generate_embedded_plan_with_computations as generate,
    EmbeddedComputationBounds, EmbeddedImageBounds, GeneratedConfigurationValue, GenerationError,
    UnsupportedPlanFeature,
};
use conduit_core::{
    seal_plan, ConfigurationEntry, ConfigurationValue, KindSemanticContract, PlanFragment,
    PlotIdentity,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    portable_expression_definition, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn program() -> PortableExpressionProgram {
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/dht11.conduit"
    ));
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "dht11-decode", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(text) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(text).unwrap()
}
fn reseal(fragment: PlanFragment) -> PlanFragment {
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    seal_plan(identity, vec![fragment])
        .fragments
        .into_iter()
        .next()
        .unwrap()
}
fn fixture() -> (PlanFragment, u32) {
    let program = program();
    let size = program.canonical_bytes().unwrap().len() as u32;
    let definition =
        portable_expression_definition(&program, conduit_core::PortTemporal::Value).unwrap();
    let mut fragment = crate::tests::sealed_current_fragment();
    let expression = &mut fragment.placements[1];
    expression.kind_id = definition.kind_id;
    expression.kind_contract_revision = definition.kind_contract_revision;
    expression.inputs = definition.inputs;
    expression.outputs = definition.outputs;
    expression.semantic_contract = KindSemanticContract {
        configuration: definition.configuration,
        laws: program.semantic_laws().unwrap(),
    };
    expression.configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text(program.canonical_hex().unwrap()),
    }];
    let input = expression.inputs[0].clone();
    fragment.placements[0].outputs[0].value_kind = input.value_kind.clone();
    fragment.connections[0].value_kind = input.value_kind;
    fragment.connections[0].sink_port_id = input.port_id;
    fragment.connections[0].byte_capacity = 1024;
    (reseal(fragment), size)
}
fn bounds(size: u32) -> EmbeddedComputationBounds {
    EmbeddedComputationBounds {
        maximum_program_bytes: size,
        maximum_total_program_bytes: size,
    }
}

#[test]
fn exact_source_expression_is_opt_in_and_preserves_fixed_image_identity() {
    let (fragment, size) = fixture();
    let lowered = lower_plan_fragment(&fragment).unwrap();
    assert_eq!(
        generate_embedded_plan(&fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING),
        Err(GenerationError::Unsupported(
            UnsupportedPlanFeature::TextConfiguration
        ))
    );
    let image = generate(
        &fragment,
        &lowered,
        EmbeddedImageBounds::HOST_TOOLING,
        bounds(size),
    )
    .unwrap();
    assert_eq!(image.plan_id, fragment.plan_id.as_str());
    let program = image
        .configuration
        .iter()
        .find(|c| c.key == "program")
        .unwrap();
    let GeneratedConfigurationValue::Text(text) = &program.value else {
        panic!("exact text")
    };
    assert_eq!(
        PortableExpressionProgram::from_canonical_hex(text)
            .unwrap()
            .canonical_bytes()
            .unwrap()
            .len(),
        size as usize
    );
    let rendered = image.render_no_alloc_firmware_module();
    assert!(rendered.contains("Text(&'static str)"));
    assert!(rendered.contains(text));
    assert_eq!(image.render_no_alloc_firmware_module(), rendered);
}
#[test]
fn program_and_total_artifact_extents_are_separately_enforced() {
    let (fragment, size) = fixture();
    let lowered = lower_plan_fragment(&fragment).unwrap();
    for (bounds, table) in [
        (
            EmbeddedComputationBounds {
                maximum_program_bytes: size - 1,
                maximum_total_program_bytes: size,
            },
            "computation program bytes",
        ),
        (
            EmbeddedComputationBounds {
                maximum_program_bytes: size,
                maximum_total_program_bytes: size - 1,
            },
            "computation total program bytes",
        ),
    ] {
        assert_eq!(
            generate(
                &fragment,
                &lowered,
                EmbeddedImageBounds::HOST_TOOLING,
                bounds
            ),
            Err(GenerationError::BoundExceeded {
                table,
                actual: u64::from(size),
                maximum: u64::from(size - 1)
            })
        );
    }
}
#[test]
fn a_resealed_plan_does_not_make_mismatched_expression_metadata_valid() {
    let (fragment, size) = fixture();
    for changed in 0..4 {
        let mut fragment = fragment.clone();
        let gear = &mut fragment.placements[1];
        match changed {
            0 => gear.kind_id = "conduitese/pure-expression/other".into(),
            1 => gear.outputs[0].temporal = conduit_core::PortTemporal::Flow { closes: true },
            2 => gear.semantic_contract.laws.clear(),
            _ => gear.configuration[0].value = ConfigurationValue::Text("00".into()),
        }
        let fragment = reseal(fragment);
        let lowered = lower_plan_fragment(&fragment).unwrap();
        assert_eq!(
            generate(
                &fragment,
                &lowered,
                EmbeddedImageBounds::HOST_TOOLING,
                bounds(size)
            ),
            Err(GenerationError::InvalidComputationConfiguration)
        );
    }
}
#[test]
fn the_expression_profile_does_not_admit_unrelated_text() {
    let (mut fragment, size) = fixture();
    fragment.placements[0].configuration[0].value =
        ConfigurationValue::Text("unrelated text".into());
    let fragment = reseal(fragment);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    assert_eq!(
        generate(
            &fragment,
            &lowered,
            EmbeddedImageBounds::HOST_TOOLING,
            bounds(size)
        ),
        Err(GenerationError::Unsupported(
            UnsupportedPlanFeature::TextConfiguration
        ))
    );
}

#[test]
fn an_exact_source_selector_is_admitted_and_its_contract_is_checked() {
    let source=format!("{}\nplot select-frame (\n >> result: Dht11DecodedResult...|\n frame: Dht11DecodedFrame...| >>\n) {{\n result >> select(Dht11DecodedResult.frame, unmatched=drop) >> frame\n}}\n",include_str!("../../../plots/device-protocols/dht11.conduit"));
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let mut profiles = ProfileCatalog::new();
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    profiles
                        .insert(conduit_plot::structured_selector_definition(
                            selector,
                            conduit_core::PortTemporal::Flow { closes: true },
                        ))
                        .unwrap();
                }
            }
        }
    }
    let expanded = expand_canonical_plot_for_authoring(&checked, "select-frame", &profiles)
        .unwrap()
        .expanded;
    let selector = &expanded.gears[0];
    let ConfigurationValue::Text(text) = &selector.configuration[0].value else {
        panic!("selector")
    };
    let size = conduit_core::StructuredSelector::from_canonical_hex(text)
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len() as u32;
    let (mut fragment, _) = fixture();
    let sink = &mut fragment.placements[1];
    sink.kind_id = selector.kind_id.clone();
    sink.kind_contract_revision = selector.kind_contract_revision.clone();
    sink.inputs = selector.inputs.clone();
    sink.outputs = selector.outputs.clone();
    sink.semantic_contract = selector.semantic_contract.clone();
    sink.configuration = selector.configuration.clone();
    fragment.placements[0].outputs[0].value_kind = selector.inputs[0].value_kind.clone();
    fragment.placements[0].outputs[0].temporal = selector.inputs[0].temporal;
    fragment.connections[0].value_kind = selector.inputs[0].value_kind.clone();
    fragment.connections[0].temporal = selector.inputs[0].temporal;
    let fragment = reseal(fragment);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let image = generate(
        &fragment,
        &lowered,
        EmbeddedImageBounds::HOST_TOOLING,
        bounds(size),
    )
    .unwrap();
    assert_eq!(image.configuration[1].key, "selector");
    let mut changed = fragment;
    changed.placements[1].kind_contract_revision = "selector/other@1".into();
    let changed = reseal(changed);
    let lowered = lower_plan_fragment(&changed).unwrap();
    assert_eq!(
        generate(
            &changed,
            &lowered,
            EmbeddedImageBounds::HOST_TOOLING,
            bounds(size)
        ),
        Err(GenerationError::InvalidComputationConfiguration)
    );
}
