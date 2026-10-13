#![cfg(feature = "plot-catalog")]
//! Ordinary checked-source and planner custody tests; the Back below is a
//! fixture and these tests do not establish DOM/scanout realization.
use conduit_core::*;
use conduit_face::*;
use conduit_planner::*;
use conduit_plot::*;
use std::collections::BTreeMap;

const VALUE: &str = "{density: compact(empty), identity: \"conduit.face/phosphor@1\", motion: none(empty), palette: phosphor(empty), text_scale_percent: 100, version: 1}";

fn port(
    name: &str,
    kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}
fn fixture(
    source: &str,
    root_name: &str,
) -> (
    CheckedSyntaxDocument,
    ExpandedAuthoringPlot,
    ProfileCatalog,
    HostAdvertisement,
) {
    let value = visual_expression_info(&VisualExpression::phosphor()).unwrap();
    let profile = value.value_type().profile().unwrap().value_kind().clone();
    let configuration =
        StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    let kind = Kind {
        kind_id: kind_id("test/configured-renderer"),
        kind_contract_revision: KindIdentity::from("test/configured-renderer@1"),
        startup_parameters: ["recipe", "expression"]
            .into_iter()
            .map(|name| FrontStartupParameter {
                name: name.into(),
                value_type: profile.clone(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        inputs: vec![port(
            "face",
            FACE_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
            port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
        ],
        configuration: ["recipe", "expression"]
            .into_iter()
            .map(|name| KindConfigurationField {
                key: name.into(),
                default_value: ConfigurationValue::Structured(configuration.clone()),
                rule: KindConfigurationRule::Structured {
                    profile: profile.clone(),
                },
            })
            .collect(),
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 2,
            max_queue_bytes: 4096,
        },
    };
    let mut profiles = ProfileCatalog::new();
    profiles.insert_kind(kind.clone()).unwrap();
    let mut startup = profiles.startup_catalog().unwrap();
    install_mask_plot_value_aliases(&mut startup).unwrap();
    startup
        .insert_structured_type("VisualExpressionInfo", visual_expression_type())
        .unwrap();
    startup
        .insert_structured_type(profile.as_str(), visual_expression_type())
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, root_name, &profiles).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/configured-test"),
        boot_id: BootId::from("boot/configured-test"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/configured@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![BackOfferBuilder::new(
            kind,
            Back {
                capability_id: CapabilityId::from("cap/configured"),
                execution_profile_id: ExecutionProfileId::from("test/configured@1"),
                implementation_id: ImplementationId::from("test/configured"),
                artifact_id: ArtifactId::from("test/configured"),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build()],
    };
    (checked, expanded, profiles, host)
}
fn source(gear_name: &str) -> String {
    format!("plot component (\n expression: VisualExpressionInfo\n >> face: Face\n show: Show >>\n interaction: FaceInteraction...| >>\n) {{\n {gear_name}: test/configured-renderer(recipe = {VALUE}, expression = expression)\n face >> {gear_name}.face\n {gear_name}.show >> show\n {gear_name}.interaction >> interaction\n}}\nplot compact-mask (\n >> face: Face\n show: Show >>\n interaction: FaceInteraction...| >>\n) {{\n invoke: component(expression = {VALUE})\n face >> invoke.face\n invoke.show >> show\n invoke.interaction >> interaction\n}}\n")
}
#[test]
fn exact_source_and_selected_plan_bind_renderer_configuration() {
    let (checked, expanded, profiles, host) = fixture(&source("render"), "compact-mask");
    let placements =
        default_expanded_placements(&expanded.expanded, std::slice::from_ref(&host)).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let boundaries = expanded
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 4096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = plan_expanded_authoring_with_options(
        &expanded,
        std::slice::from_ref(&host),
        &placements,
        &[],
        options(),
        &boundaries,
    )
    .unwrap();
    let planned = PlannedMaskPlot::admit(&MaskPlot::admit(&expanded).unwrap(), &plan).unwrap();
    let selection = || ConfiguredMaskPlanningInputs {
        hosts: std::slice::from_ref(&host),
        placements: &placements,
        bases: &[],
        options: options(),
        boundary_limits: &boundaries,
    };
    let checked_plan = prepare_checked_source_plan(
        &checked,
        &expanded,
        &profiles,
        &CanonicalBackCatalog::new(),
        &plan,
        selection(),
    )
    .unwrap();
    assert_eq!(checked_plan.plan(), &plan);
    assert_eq!(checked_plan.expanded(), &expanded);
    let mut forged_plan = plan.clone();
    forged_plan.plan_id = PlanId::from("plan/forged-custody");
    assert!(matches!(
        prepare_checked_source_plan(
            &checked,
            &expanded,
            &profiles,
            &CanonicalBackCatalog::new(),
            &forged_plan,
            selection(),
        ),
        Err(CheckedSourcePlanRefusal::PlanReproduction)
    ));
    let mut changed_host = host.clone();
    changed_host.boot_id = BootId::from("boot/replaced-input-host");
    assert!(matches!(
        prepare_checked_source_plan(
            &checked,
            &expanded,
            &profiles,
            &CanonicalBackCatalog::new(),
            &plan,
            CheckedSourcePlanInputs {
                hosts: std::slice::from_ref(&changed_host),
                placements: &placements,
                bases: &[],
                options: options(),
                boundary_limits: &boundaries,
            },
        ),
        Err(CheckedSourcePlanRefusal::PlanReproduction)
    ));
    let prepared = prepare_configured_mask(
        &checked,
        &expanded,
        &profiles,
        &CanonicalBackCatalog::new(),
        &planned,
        selection(),
        &kind_id("test/configured-renderer"),
    )
    .unwrap();
    assert_eq!(
        prepared.component.checked_plot_id,
        checked
            .plots
            .iter()
            .find(|plot| plot.name == "component")
            .unwrap()
            .checked_plot_id
    );
    assert_eq!(
        prepared.renderer.gear_id.as_str(),
        "compact-mask/invoke/render"
    );
    assert_eq!(
        prepared.component_invocation_path,
        ["compact-mask", "invoke"]
    );
    assert_ne!(
        prepared.component.checked_plot_id,
        planned.mask.plot_identity.checked_plot_id
    );
    for mutate in [
        (|plan: &mut Plan| {
            plan.fragments[0].placements[0].implementation_id =
                ImplementationId::from("foreign/renderer")
        }) as fn(&mut Plan),
        |plan: &mut Plan| {
            plan.fragments[0].placements[0].execution_profile_id =
                ExecutionProfileId::from("foreign/profile")
        },
        |plan: &mut Plan| {
            plan.fragments[0].host_id = HostId::from("host/foreign");
            for placement in &mut plan.fragments[0].placements {
                placement.host_id = HostId::from("host/foreign");
            }
        },
    ] {
        let mut foreign = planned.clone();
        mutate(&mut foreign.plan);
        // Canonically reseal a foreign selection; canonical validity is not offer custody.
        foreign.plan = seal_plan_with_realization_backs_and_completion(
            PlotIdentity {
                source_document_id: foreign.plan.source_document_id.clone(),
                checked_plot_id: foreign.plan.checked_plot_id.clone(),
                expanded_plot_id: foreign.plan.expanded_plot_id.clone(),
            },
            foreign.plan.completion_policy,
            foreign.plan.realization_backs.clone(),
            foreign.plan.fragments,
        );
        assert!(verify_plan(&foreign.plan));
        assert!(matches!(
            prepare_configured_mask(
                &checked,
                &expanded,
                &profiles,
                &CanonicalBackCatalog::new(),
                &foreign,
                selection(),
                &kind_id("test/configured-renderer")
            ),
            Err(ConfiguredMaskRefusal::BackPreparation)
        ));
    }
}

type ConfiguredMaskEvidence = (
    ConfiguredComponentSource,
    Vec<u8>,
    Vec<u8>,
    PlanId,
    ExpandedPlotId,
);

fn evidence(
    source: &str,
    root_name: &str,
) -> Result<ConfiguredMaskEvidence, ConfiguredMaskRefusal> {
    let (checked, expanded, profiles, host) = fixture(source, root_name);
    let placements =
        default_expanded_placements(&expanded.expanded, std::slice::from_ref(&host)).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let boundaries = expanded
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 4096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = plan_expanded_authoring_with_options(
        &expanded,
        std::slice::from_ref(&host),
        &placements,
        &[],
        options(),
        &boundaries,
    )
    .unwrap();
    let planned = PlannedMaskPlot::admit(&MaskPlot::admit(&expanded).unwrap(), &plan).unwrap();
    let selection = || ConfiguredMaskPlanningInputs {
        hosts: std::slice::from_ref(&host),
        placements: &placements,
        bases: &[],
        options: options(),
        boundary_limits: &boundaries,
    };
    let prepared = prepare_configured_mask(
        &checked,
        &expanded,
        &profiles,
        &CanonicalBackCatalog::new(),
        &planned,
        selection(),
        &kind_id("test/configured-renderer"),
    )?;
    Ok((
        prepared.component,
        prepared.recipe.canonical_value().to_vec(),
        prepared.expression.canonical_value().to_vec(),
        planned.plan.plan_id.clone(),
        expanded.expanded.expanded_plot_id,
    ))
}

#[test]
fn two_roots_share_checked_component_fore_and_recipe_but_not_realization() {
    let compact = source("render");
    let spacious_root = compact.split("plot compact-mask").nth(1).unwrap();
    let both = format!(
        "{compact}plot spacious-mask{}",
        spacious_root.replace("density: compact(empty)", "density: spacious(empty)")
    );
    let a = evidence(&both, "compact-mask").unwrap();
    let b = evidence(&both, "spacious-mask").unwrap();
    assert_eq!(a.0.source_document_id, b.0.source_document_id);
    assert_eq!(a.0.checked_plot_id, b.0.checked_plot_id);
    assert_eq!(a.0.fore_id, b.0.fore_id);
    assert_eq!(a.1, b.1);
    assert_ne!(a.2, b.2);
    assert_ne!(a.3, b.3);
    assert_ne!(a.4, b.4);
}

#[test]
fn explicit_component_forwarding_and_slash_plot_ancestry_are_checked() {
    let ordinary = source("render");
    let slash = ordinary
        .replace("plot component (", "plot component/layout (")
        .replace("invoke: component(", "invoke: component/layout(");
    assert!(evidence(&slash, "compact-mask").is_ok());
    let forged = ordinary.replace(
        "expression = expression)",
        &format!("expression = {VALUE})"),
    );
    // Equal expression bytes do not replace direct checked caller forwarding.
    assert!(matches!(
        evidence(&forged, "compact-mask"),
        Err(ConfiguredMaskRefusal::Configuration)
    ));
}
