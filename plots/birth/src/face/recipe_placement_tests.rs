//! Source/Plan configuration and structural placement proof, not native scanout.
use super::*;
use conduit_conduitos_mask_offer::*;
use conduit_core::*;
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn configuration(layout: bool, recipe: CrecheComponentRecipe) -> ConfiguredMaskConfiguration {
    let provider = BaseProviderEntry {
        base_id: "collection/framebuffer".into(),
        provider_instance_id: "collection/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![resource_offer("collection/surface", SHOW_RESOURCE_CLASS, 1)],
    };
    let host = if layout {
        native_layout_host_advertisement(
            &"host/birth".into(),
            &"boot/birth".into(),
            1,
            &provider,
            NativeMaskStorageProfile::Standard,
        )
    } else {
        native_host_advertisement(&"host/birth".into(), &"boot/birth".into(), 1, &provider)
    };
    let (_, configuration) = prepare_creche_stage_from_offer(&host, recipe).unwrap();
    assert_eq!(configuration.layout.is_some(), layout);
    configuration
}
fn info(bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(bytes).unwrap()
}
fn selection_recipe(
    configuration: &ConfiguredMaskConfiguration,
    typed: bool,
) -> StructuredInfoValue {
    let recipe =
        ComponentRecipeInfo::from_structured(info(configuration.recipe.canonical_value())).unwrap();
    let regions = recipe.regions().iter().map(|region| {
        let selector = if region.identity() == "selection" {
            if typed {
                ComponentSubjectSelector::collection(16, "independent".into()).unwrap()
            } else {
                ComponentSubjectSelector::role("Collection".into()).unwrap()
            }
        } else {
            region.selector().clone()
        };
        ComponentRegionInfo::new(
            *region.group(),
            region.identity().clone(),
            *region.maximum_subjects(),
            selector,
        )
        .unwrap()
    });
    ComponentRecipeInfo::new(
        *recipe.gutter(),
        recipe.identity().clone(),
        *recipe.maximum_columns(),
        *recipe.reflow_width(),
        BoundedSequence::try_from_iter(regions).unwrap(),
        *recipe.version(),
    )
    .unwrap()
    .into_structured()
    .unwrap()
}
fn legacy_face(mut face: Face) -> Face {
    // The historical adapter had none of these canonical collection facts.
    // Preserve every other public field; actual pre-change Source is separately
    // compared in the ignored, source-bound before/after proof packet.
    face.properties.retain(|p| {
        !matches!(
            p.name.as_str(),
            "choice-multiplicity" | "item-count" | "order"
        )
    });
    Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .unwrap()
    .with_interaction_context(face.interaction_context)
    .unwrap()
    .with_composition(face.composition)
    .unwrap()
}
fn assert_fallback(
    configuration: &ConfiguredMaskConfiguration,
    recipe: StructuredInfoValue,
    face: &Face,
) {
    if let Some(layout) = &configuration.layout {
        let prepared =
            prepare_component_layout(info(layout.canonical_value()), recipe, face).unwrap();
        assert_eq!(prepared.face(), face);
        assert_eq!(
            prepared.recipe().reading,
            plan_face_utterances(face).unwrap()
        );
        for candidate in prepared.structure().candidates().iter() {
            assert_eq!(
                candidate
                    .nodes()
                    .iter()
                    .filter(|n| matches!(n, ComponentLayoutNode::Fallback))
                    .count(),
                1
            );
        }
    } else {
        let prepared = prepare_component_recipe(recipe, face).unwrap();
        assert_eq!(prepared.reading, plan_face_utterances(face).unwrap());
    }
}
#[test]
fn legacy_missing_typed_facts_keeps_full_fallback_but_does_not_preserve_selection_placement() {
    for layout in [false, true] {
        for recipe in [
            CrecheComponentRecipe::SingleColumn,
            CrecheComponentRecipe::ResponsiveColumns,
        ] {
            let configuration = configuration(layout, recipe);
            for query in ["", "Clock", "Speaker", "none"] {
                let mut draft = draft();
                if !query.is_empty() {
                    draft.search_plots(draft.revision(), query).unwrap();
                }
                let face = legacy_face(draft.host_owned_face(&host_owned_basis()).unwrap());
                let old = prepare_component_recipe(selection_recipe(&configuration, false), &face)
                    .unwrap();
                let proposed =
                    prepare_component_recipe(selection_recipe(&configuration, true), &face)
                        .unwrap();
                assert_eq!(old.reading, proposed.reading);
                for (index, subject) in face.subjects.iter().enumerate() {
                    if subject.role == FaceRole::Collection {
                        assert!(old.subject_regions[index].is_some());
                        assert_eq!(proposed.subject_regions[index], None);
                    } else {
                        assert_eq!(old.subject_regions[index], proposed.subject_regions[index]);
                    }
                }
                assert_fallback(
                    &configuration,
                    selection_recipe(&configuration, true),
                    &face,
                );
            }
        }
    }
}

#[test]
fn current_typed_choice_facts_preserve_old_role_placement_for_both_documents_and_roots() {
    for layout in [false, true] {
        for recipe in [
            CrecheComponentRecipe::SingleColumn,
            CrecheComponentRecipe::ResponsiveColumns,
        ] {
            let configuration = configuration(layout, recipe);
            let authored =
                ComponentRecipeInfo::from_structured(info(configuration.recipe.canonical_value()))
                    .unwrap();
            let groups: alloc::vec::Vec<_> =
                authored.regions().iter().map(|r| *r.group()).collect();
            let expected_groups = match recipe {
                CrecheComponentRecipe::SingleColumn => vec![0, 0, 0, 0, 0, 0],
                CrecheComponentRecipe::ResponsiveColumns => vec![0, 0, 1, 1, 1, 2],
            };
            assert_eq!(groups, expected_groups);
            assert_eq!(
                authored
                    .regions()
                    .iter()
                    .map(|r| *r.maximum_subjects())
                    .collect::<alloc::vec::Vec<_>>(),
                vec![1, 1, 1, 1, 16, 20]
            );
            if let Some(layout) = &configuration.layout {
                let structure =
                    ComponentLayoutInfo::from_structured(info(layout.canonical_value())).unwrap();
                assert_eq!(
                    structure.candidates().len(),
                    match recipe {
                        CrecheComponentRecipe::SingleColumn => 1,
                        CrecheComponentRecipe::ResponsiveColumns => 2,
                    }
                );
            }
            assert!(
                matches!(authored.regions().iter().find(|r|r.identity()=="selection").unwrap().selector(),
                ComponentSubjectSelector::Collection(binding) if binding.mode()=="independent" && *binding.maximum_items()==16)
            );
            for query in ["", "Clock", "Speaker", "none"] {
                let mut draft = draft();
                if !query.is_empty() {
                    draft.search_plots(draft.revision(), query).unwrap();
                }
                let face = draft.host_owned_face(&host_owned_basis()).unwrap();
                let old = prepare_component_recipe(selection_recipe(&configuration, false), &face)
                    .unwrap();
                let typed = prepare_component_recipe(selection_recipe(&configuration, true), &face)
                    .unwrap();
                assert_eq!(
                    old.subject_regions, typed.subject_regions,
                    "query={query:?} layout={layout}"
                );
                assert_eq!(old.reading, typed.reading);
                let actual =
                    prepare_component_recipe(info(configuration.recipe.canonical_value()), &face)
                        .unwrap();
                assert_eq!(actual.subject_regions, typed.subject_regions);
                assert_fallback(
                    &configuration,
                    info(configuration.recipe.canonical_value()),
                    &face,
                );
                for collection in face
                    .subjects
                    .iter()
                    .filter(|s| s.role == FaceRole::Collection)
                {
                    let members: alloc::vec::Vec<_> = face
                        .relationships
                        .iter()
                        .filter(|r| {
                            r.source == collection.identity
                                && r.kind == FaceRelationshipKind::Contains
                        })
                        .collect();
                    assert!(
                        face.properties
                            .iter()
                            .any(|p| p.subject == collection.identity
                                && p.name == "choice-multiplicity"
                                && p.value == FacePropertyValue::Text("independent".into()))
                    );
                    assert!(
                        face.properties
                            .iter()
                            .any(|p| p.subject == collection.identity
                                && p.name == "item-count"
                                && p.value == FacePropertyValue::Count(members.len() as u64))
                    );
                    for (index, member) in members.iter().enumerate() {
                        assert!(face.properties.iter().any(|p| p.subject == member.target
                            && p.name == "order"
                            && p.value == FacePropertyValue::Count(index as u64)));
                    }
                }
                // Complete original facts/actions are borrowed by either recipe;
                // this is structural admission, not execution or pixel geometry.
                assert_fallback(
                    &configuration,
                    selection_recipe(&configuration, true),
                    &face,
                );
            }
        }
    }
}

#[test]
fn maximum_current_inventory_keeps_complete_structural_placement_and_reading() {
    let choices = (0..conduit_body::MAX_BODY_PLOTS)
        .map(|index| crate::BirthPlotChoice {
            title: format!("界 {index}"),
            search_text: "finite current inventory".into(),
            plot: conduit_body::ResidentPlot::new(
                format!("source/{index}").into(),
                format!("checked/{index}").into(),
            ),
            refusal: None,
            selected: index % 2 == 0,
        })
        .collect();
    let draft = BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    let face = draft.host_owned_face(&host_owned_basis()).unwrap();
    assert_eq!(
        face.subjects
            .iter()
            .filter(|s| s.role == FaceRole::Plot)
            .count(),
        16
    );
    let before = face.clone();
    for layout in [false, true] {
        for recipe in [
            CrecheComponentRecipe::SingleColumn,
            CrecheComponentRecipe::ResponsiveColumns,
        ] {
            let configuration = configuration(layout, recipe);
            let old =
                prepare_component_recipe(selection_recipe(&configuration, false), &face).unwrap();
            let actual =
                prepare_component_recipe(info(configuration.recipe.canonical_value()), &face)
                    .unwrap();
            assert_eq!(old.subject_regions, actual.subject_regions);
            assert_eq!(old.reading, actual.reading);
            assert_fallback(
                &configuration,
                info(configuration.recipe.canonical_value()),
                &face,
            );
        }
    }
    assert_eq!(face, before);
}
