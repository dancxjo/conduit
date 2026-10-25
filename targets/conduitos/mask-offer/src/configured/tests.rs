mod typography;
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
fn host() -> HostAdvertisement {
    let provider = BaseProviderEntry {
        base_id: "native/component/framebuffer".into(),
        provider_instance_id: "native/component/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![resource_offer(
            "native/component/surface",
            SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    renderer_host(
        Adapter::Native,
        &HostId::from("native/component"),
        &BootId::from("native/boot"),
        1,
        Some(&provider),
    )
}

#[test]
fn native_checked_component_two_looks_and_recipes() {
    let host = host();
    for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
        let (compact, a) =
            prepare_component_stage_from_offer(&host, ComponentMaskLook::Compact, recipe).unwrap();
        let (spacious, b) =
            prepare_component_stage_from_offer(&host, ComponentMaskLook::Spacious, recipe).unwrap();
        assert_eq!(a.component, b.component);
        assert_eq!(a.recipe, b.recipe);
        assert_ne!(a.look, b.look);
        assert_ne!(
            compact.planned_mask.plan.plan_id,
            spacious.planned_mask.plan.plan_id
        );
        assert_eq!(
            a.implementation_id.as_str(),
            "face/renderer-conduitos-component@1"
        );
    }
    let mut foreign = host;
    foreign
        .capabilities
        .retain(|offer| offer.capability_id.as_str() != "renderer-conduitos-component");
    assert!(
        prepare_component_stage_from_offer(
            &foreign,
            ComponentMaskLook::Compact,
            TodoComponentRecipe::Flat
        )
        .is_err()
    );
}

#[test]
fn creche_checked_roots_select_structure_independently_of_look() {
    let host = host();
    let mut configurations = Vec::new();
    for recipe in [
        CrecheComponentRecipe::SingleColumn,
        CrecheComponentRecipe::ResponsiveColumns,
    ] {
        let (stage, configuration) = prepare_creche_stage_from_offer(&host, recipe).unwrap();
        assert_eq!(stage.planned_mask.mask.plot_name, recipe.root());
        configurations.push(configuration);
    }
    assert_eq!(configurations[0].look, configurations[1].look);
    assert_ne!(configurations[0].recipe, configurations[1].recipe);
    let mut foreign = host;
    foreign
        .capabilities
        .retain(|offer| offer.capability_id.as_str() != "renderer-conduitos-component");
    assert!(
        prepare_creche_stage_from_offer(&foreign, CrecheComponentRecipe::ResponsiveColumns)
            .is_err()
    );
}
#[test]
fn selection_axes_reproduce_one_checked_component_in_both_domains() {
    let host = host();
    for creche in [false, true] {
        for grouped in [false, true] {
            let mut baseline: Option<ConfiguredMaskConfiguration> = None;
            let mut plan_ids = Vec::new();
            for density in [VisualDensity::Compact, VisualDensity::Spacious] {
                for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
                    let selection = ComponentVisualSelection { density, palette };
                    let (stage, configuration) = if creche {
                        prepare_selected_creche_stage_from_offer(
                            &host,
                            if grouped {
                                CrecheComponentRecipe::ResponsiveColumns
                            } else {
                                CrecheComponentRecipe::SingleColumn
                            },
                            selection,
                        )
                    } else {
                        prepare_selected_component_stage_from_offer(
                            &host,
                            selection,
                            if grouped {
                                TodoComponentRecipe::Grouped
                            } else {
                                TodoComponentRecipe::Flat
                            },
                        )
                    }
                    .unwrap();
                    let resource = configuration.requested_look().unwrap();
                    let LookVisualFacetOption::Some(visual) = resource.visual() else {
                        panic!("visual facet")
                    };
                    let expression = visual_expression_from_info(
                        visual.expression().clone().into_structured().unwrap(),
                    )
                    .unwrap();
                    assert_eq!((expression.density, expression.palette), (density, palette));
                    assert_eq!(
                        expression.text_scale_percent,
                        if density == VisualDensity::Compact {
                            100
                        } else {
                            125
                        }
                    );
                    if let Some(first) = &baseline {
                        assert_eq!(first.component, configuration.component);
                        assert_eq!(first.recipe, configuration.recipe);
                    } else {
                        baseline = Some(configuration);
                    }
                    assert!(!plan_ids.contains(&stage.planned_mask.plan.plan_id));
                    plan_ids.push(stage.planned_mask.plan.plan_id);
                }
            }
        }
    }
}
