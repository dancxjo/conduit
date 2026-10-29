#![cfg(feature = "plot-catalog")]
//! Checked source/Plan custody only; no native Back is selected by this fixture.
use conduit_core::*;
use conduit_face::*;
use conduit_planner::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::*;
use std::collections::BTreeMap;
const EXPRESSION: &str = r#"{generative: some({register: neutral(empty), verbosity: brief(empty)}), identity: "conduit.look/phosphor@1", spoken: some({delivery: neutral(empty), verbosity: brief(empty)}), version: 2, visual: some({expression: {density: compact(empty), identity: "conduit.look/phosphor@1", motion: none(empty), palette: phosphor(empty), text_scale_percent: 100, typography: editorial(empty), version: 2}, tokens: {background: {blue: 11, green: 7, red: 5}, bootstrap_surface: {blue: 22, green: 13, red: 9}, emphasis: {blue: 37, green: 163, red: 233}, failure: {blue: 114, green: 114, red: 255}, focus: {blue: 0, green: 196, red: 244}, focus_width_px: 3, line_height_percent: 150, muted: {blue: 25, green: 76, red: 140}, radius_control_px: 6, radius_panel_px: 9, reading_paper: {blue: 28, green: 18, red: 12}, space_control_block_px: 7, space_control_inline_px: 10, space_panel_px: 13, space_unit_px: 4, structure_primary: {blue: 246, green: 216, red: 13}, structure_secondary: {blue: 135, green: 31, red: 10}, success: {blue: 155, green: 214, red: 99}, surface: {blue: 22, green: 13, red: 9}, text_primary: {blue: 247, green: 210, red: 147}, text_secondary: {blue: 201, green: 142, red: 87}, type_body_px: 16, type_small_px: 14, warning: {blue: 113, green: 189, red: 242}, workbench_canvas: {blue: 11, green: 7, red: 5}}})}"#;
const RECIPE: &str = "{gutter: 12, identity: \"test/recipe@1\", maximum_columns: 1, reflow_width: 720, regions: [{group: 0, identity: \"collection\", maximum_subjects: 20, selector: role(\"Collection\")}], version: 1}";
const LAYOUT: &str = "{candidates: [{nodes: [column({block_alignment: stretch(empty), break_policy: accessible_continuation(empty), children: [{block_span: 1, block_track: 0, inline_span: 1, inline_track: 0, node: 1}, {block_span: 1, block_track: 0, inline_span: 1, inline_track: 0, node: 2}], gap_em_sixteenths: 8, inline_grow: 0, inline_alignment: stretch(empty), padding_em_sixteenths: 8, inline_shrink: 1, minimum_inline_em_sixteenths: 0}), region(0), fallback(empty)], root: 0}], identity: \"test/layout@1\", recipe_identity: \"test/recipe@1\", version: 1}";
fn fixture(
    source: &str,
    root_name: &str,
) -> (
    CheckedSyntaxDocument,
    ExpandedAuthoringPlot,
    ProfileCatalog,
    HostAdvertisement,
    CanonicalBackCatalog,
) {
    fixture_with(source, root_name, None, |_| {})
}
fn fixture_with(
    source: &str,
    root_name: &str,
    back_source: Option<&str>,
    alter_kind: fn(&mut Kind),
) -> (
    CheckedSyntaxDocument,
    ExpandedAuthoringPlot,
    ProfileCatalog,
    HostAdvertisement,
    CanonicalBackCatalog,
) {
    let mut kind = layout_component_mask_kind(4096);
    kind.kind_id = kind_id("test/configured-renderer");
    kind.kind_contract_revision = KindIdentity::from("test/layout-renderer@1");
    alter_kind(&mut kind);
    let mut profiles = ProfileCatalog::new();
    profiles.insert_kind(kind.clone()).unwrap();
    if back_source.is_some() {
        profiles
            .insert_kind(layout_component_mask_kind(4096))
            .unwrap();
    }
    let mut startup = profiles.startup_catalog().unwrap();
    install_mask_plot_value_aliases(&mut startup).unwrap();
    for (name, schema) in [
        ("LookInfo", look_type()),
        ("ComponentRecipeInfo", component_recipe_type()),
        ("ComponentLayoutInfo", component_layout_type()),
    ] {
        let profile = schema.profile().unwrap().value_kind().clone();
        startup
            .insert_structured_type(name, schema.clone())
            .unwrap();
        startup
            .ensure_structured_type(profile.as_str(), schema)
            .unwrap();
    }
    let mut backs = CanonicalBackCatalog::new();
    if let Some(source) = back_source {
        let back = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        backs
            .insert(&layout_component_mask_kind(4096), &back, "layout-back")
            .unwrap();
    }
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring_with_backs(&checked, root_name, &profiles, &backs)
            .unwrap();
    let host = HostAdvertisement {
        declared_name: None,
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
    (checked, expanded, profiles, host, backs)
}
fn evidence(
    source: &str,
    root_name: &str,
) -> Result<ConfiguredMaskConfiguration, ConfiguredMaskRefusal> {
    evidence_with(source, root_name, None, |_| {})
}
fn evidence_with(
    source: &str,
    root_name: &str,
    back_source: Option<&str>,
    alter_kind: fn(&mut Kind),
) -> Result<ConfiguredMaskConfiguration, ConfiguredMaskRefusal> {
    let (checked, expanded, profiles, host, backs) =
        fixture_with(source, root_name, back_source, alter_kind);
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
    let encoded = serde_json::to_vec(&planned).unwrap();
    let restored: PlannedMaskPlot = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        restored, planned,
        "compact transport preserves complete Plan"
    );
    assert_eq!(restored.plan.plan_id, plan.plan_id);
    eprintln!("configured Mask Plan transport: {} bytes", encoded.len());

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
        &backs,
        &planned,
        selection(),
        &kind_id("test/configured-renderer"),
    )?;
    Ok(prepared.configuration())
}

fn source() -> String {
    format!(
        "plot component (\n look: LookInfo\n >> face: Face\n show: Show >>\n interaction: FaceInteraction...| >>\n) {{\n render: test/configured-renderer(label = \"component\", recipe = {RECIPE}, layout = {LAYOUT}, look = look)\n face >> render.face\n render.show >> show\n render.interaction >> interaction\n}}\nplot root (\n >> face: Face\n show: Show >>\n interaction: FaceInteraction...| >>\n) {{\n invoke: component(look = {EXPRESSION})\n face >> invoke.face\n invoke.show >> show\n invoke.interaction >> interaction\n}}\n"
    )
}
#[test]
fn complete_look_contracts_are_distinct_and_require_typed_layout() {
    let old = component_mask_kind(4096);
    let new = layout_component_mask_kind(4096);
    assert_eq!(old.kind_id, new.kind_id);
    assert_eq!(
        old.kind_contract_revision.as_str(),
        "conduit.face/component-mask@4"
    );
    assert_eq!(
        new.kind_contract_revision.as_str(),
        "conduit.face/component-mask@5"
    );
    assert!(!old.configuration.iter().any(|f| f.key == "layout"));
    assert!(new
        .startup_parameters
        .iter()
        .any(|p| p.name == "layout" && !p.has_default));
    for kind in [&old, &new] {
        assert!(!kind
            .startup_parameters
            .iter()
            .any(|p| p.name == "expression"));
        let look = kind
            .startup_parameters
            .iter()
            .find(|p| p.name == "look")
            .unwrap();
        assert!(!look.has_default);
        assert_eq!(
            look.value_type,
            look_type().profile().unwrap().value_kind().clone()
        );
    }
    assert_eq!(new.inputs, old.inputs);
    assert_eq!(new.outputs, old.outputs);
}
#[test]
fn exact_component_invocation_retains_both_literals() {
    let prepared = evidence(&source(), "root").unwrap();
    assert!(prepared.layout.is_some());
    assert_eq!(prepared.invocation_path, ["root", "invoke"]);
    assert_eq!(
        prepared.component.checked_plot_id,
        fixture(&source(), "root")
            .0
            .plots
            .iter()
            .find(|p| p.name == "component")
            .unwrap()
            .checked_plot_id
    );
}
#[test]
fn layout_literal_at_outer_invocation_cannot_replace_component_literal() {
    let source = source()
        .replace(
            " look: LookInfo",
            " look: LookInfo\n layout: ComponentLayoutInfo",
        )
        .replace(
            &format!("layout = {LAYOUT}, look = look"),
            "layout = layout, look = look",
        )
        .replace(
            &format!("component(look = {EXPRESSION})"),
            &format!("component(layout = {LAYOUT}, look = {EXPRESSION})"),
        );
    assert!(matches!(
        evidence(&source, "root"),
        Err(ConfiguredMaskRefusal::LayoutNotSourceLiteral)
    ));
}

fn back_source() -> String {
    "plot layout-back (\n label: Text\n recipe: ComponentRecipeInfo\n look: LookInfo\n layout: ComponentLayoutInfo\n >> face: Face\n show: Show >>\n interaction: FaceInteraction...| >>\n) {\n render: test/configured-renderer(label = label, recipe = recipe, layout = layout, look = look)\n face >> render.face\n render.show >> show\n render.interaction >> interaction\n}\n".into()
}
#[test]
fn authenticated_back_forwards_both_literals_without_substitution() {
    let source = source().replace("test/configured-renderer(", "face/component-mask(");
    let back = back_source();
    let prepared = evidence_with(&source, "root", Some(&back), |_| {}).unwrap();
    assert!(prepared.layout.is_some());
    assert_eq!(prepared.invocation_path, ["root", "invoke"]);
    let substituted = back.replace("layout = layout", &format!("layout = {LAYOUT}"));
    assert!(matches!(
        evidence_with(&source, "root", Some(&substituted), |_| {}),
        Err(ConfiguredMaskRefusal::LayoutNotSourceLiteral)
    ));
    let substituted_recipe = back.replace("recipe = recipe", &format!("recipe = {RECIPE}"));
    assert!(matches!(
        evidence_with(&source, "root", Some(&substituted_recipe), |_| {}),
        Err(ConfiguredMaskRefusal::RecipeNotSourceLiteral)
    ));
}
#[test]
fn declared_required_layout_and_complete_look_revision_are_enforced() {
    assert!(matches!(
        evidence_with(&source(), "root", None, |kind| {
            kind.startup_parameters
                .iter_mut()
                .find(|p| p.name == "layout")
                .unwrap()
                .has_default = true;
        }),
        Err(ConfiguredMaskRefusal::Configuration)
    ));
    let source = source().replace(&format!("layout = {LAYOUT}, "), "");
    assert!(matches!(
        evidence_with(&source, "root", None, |kind| {
            kind.startup_parameters.retain(|p| p.name != "layout");
            kind.configuration.retain(|p| p.key != "layout");
            kind.kind_contract_revision = KindIdentity::from("conduit.face/component-mask@5");
        }),
        Err(ConfiguredMaskRefusal::Configuration)
    ));
}

#[test]
fn equal_sibling_literals_and_expression_substitution_do_not_confer_custody() {
    let original = source();
    let sibling = original
        .split("plot root")
        .next()
        .unwrap()
        .replace("plot component (", "plot sibling (");
    let delegated = original
        .replace(
            " look: LookInfo",
            " look: LookInfo\n layout: ComponentLayoutInfo",
        )
        .replace(
            &format!("layout = {LAYOUT}, look = look"),
            "layout = layout, look = look",
        )
        .replace(
            &format!("component(look = {EXPRESSION})"),
            &format!("component(layout = {LAYOUT}, look = {EXPRESSION})"),
        );
    assert!(matches!(
        evidence(&format!("{delegated}{sibling}"), "root"),
        Err(ConfiguredMaskRefusal::LayoutNotSourceLiteral)
    ));
    let back = back_source().replace("look = look", &format!("look = {EXPRESSION}"));
    assert!(matches!(
        evidence_with(
            &original.replace("test/configured-renderer(", "face/component-mask("),
            "root",
            Some(&back),
            |_| {}
        ),
        Err(ConfiguredMaskRefusal::Configuration)
    ));
}

#[test]
fn named_visual_presets_match_across_distinct_checked_components() {
    let todo = TODO_LAYOUT_MASK_SOURCE.replace("face/component-mask(", "test/configured-renderer(");
    let creche =
        CRECHE_LAYOUT_MASK_SOURCE.replace("face/component-mask(", "test/configured-renderer(");
    let mut first_structure = None;
    for look in [
        ComponentMaskLook::Compact,
        ComponentMaskLook::CompactEditorial,
        ComponentMaskLook::Spacious,
        ComponentMaskLook::SpaciousPhosphor,
    ] {
        let selected = ComponentVisualSelection::from(look);
        let a = evidence(&todo, selected.todo_root(TodoComponentRecipe::Grouped)).unwrap();
        let b = evidence(
            &creche,
            selected.creche_root(CrecheComponentRecipe::ResponsiveColumns),
        )
        .unwrap();
        assert_ne!(a.recipe.canonical_value(), b.recipe.canonical_value());
        let structure = (
            (
                a.component.fore_id.clone(),
                a.recipe.canonical_value().to_vec(),
                a.layout.as_ref().unwrap().canonical_value().to_vec(),
            ),
            (
                b.component.fore_id.clone(),
                b.recipe.canonical_value().to_vec(),
                b.layout.as_ref().unwrap().canonical_value().to_vec(),
            ),
        );
        if let Some(first) = &first_structure {
            assert_eq!(
                &structure, first,
                "Look must preserve each checked structure"
            );
        } else {
            first_structure = Some(structure);
        }
        let resource = a.requested_look().unwrap();
        let LookVisualFacetOption::Some(visual) = resource.visual() else {
            panic!("visual facet")
        };
        let actual =
            visual_expression_from_info(visual.expression().clone().into_structured().unwrap())
                .unwrap();
        assert_eq!(actual.typography, VisualExpression::phosphor().typography);
        let reference = if actual.palette == VisualPalette::Phosphor {
            phosphor_look().unwrap()
        } else {
            editorial_look().unwrap()
        };
        let LookVisualFacetOption::Some(reference_visual) = reference.visual() else {
            panic!("reference visual facet")
        };
        assert_eq!(visual.tokens(), reference_visual.tokens());
        assert_eq!(resource.spoken(), reference.spoken());
        assert_eq!(resource.generative(), reference.generative());
        if look == ComponentMaskLook::Compact {
            assert_eq!(resource, reference);
        }
        let todo_expression = a.look.canonical_value();
        let creche_expression = b.look.canonical_value();
        assert_eq!(
            todo_expression, creche_expression,
            "same named Look must not change policy with its component: {look:?}"
        );
    }
}

#[test]
fn retained_component_roots_share_exact_named_visual_policies() {
    fn prepare(source: &str, root: &str) -> ConfiguredMaskConfiguration {
        evidence_with(source, root, None, |kind| {
            *kind = component_mask_kind(4096);
            kind.kind_id = kind_id("test/configured-renderer");
        })
        .unwrap()
    }
    let todo =
        TODO_COMPONENT_MASK_SOURCE.replace("face/component-mask(", "test/configured-renderer(");
    let creche =
        CRECHE_COMPONENT_MASK_SOURCE.replace("face/component-mask(", "test/configured-renderer(");
    for look in [
        ComponentMaskLook::Compact,
        ComponentMaskLook::CompactEditorial,
        ComponentMaskLook::Spacious,
        ComponentMaskLook::SpaciousPhosphor,
    ] {
        let selected = ComponentVisualSelection::from(look);
        let expected = prepare(
            &creche,
            selected.creche_root(CrecheComponentRecipe::ResponsiveColumns),
        );
        for recipe in [
            TodoComponentRecipe::Flat,
            TodoComponentRecipe::Grouped,
            TodoComponentRecipe::RequiredItem,
        ] {
            let actual = prepare(&todo, selected.todo_root(recipe));
            assert_eq!(
                actual.look.canonical_value(),
                expected.look.canonical_value(),
                "same named policy for {look:?} / {recipe:?}"
            );
        }
    }
}
