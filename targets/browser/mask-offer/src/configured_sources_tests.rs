//! Exact registered document/Look custody; no live owner or Show is claimed.
use crate::*;
use conduit_core::*;
use conduit_face::*;

fn host() -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "browser/cross-component".into(),
        boot_id: "browser/cross-component/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "browser".into(),
        bases: vec![],
        resources: vec![resource_offer("browser/face", FACE_RESOURCE_CLASS, 1)],
        planner_capabilities: vec![],
        capabilities: vec![configured_offer()],
    }
}
fn configured(source: &str, root: &str) -> (PlannedMaskPlot, ConfiguredMaskConfiguration) {
    let (planned, config) = planned_mask_configuration(
        &host(),
        source,
        root,
        Some(OwnerCarrierForeLimits {
            face_bytes: 60 * 1024,
            show_bytes: 60 * 1024,
        }),
    )
    .unwrap();
    (planned, config.unwrap())
}
#[test]
fn independent_patchbay_recipe_reuses_exact_todo_looks_and_back_custody() {
    for look in [
        ComponentMaskLook::Compact,
        ComponentMaskLook::Spacious,
        ComponentMaskLook::CompactEditorial,
        ComponentMaskLook::SpaciousPhosphor,
    ] {
        let (_, todo) = configured(
            TODO_COMPONENT_MASK_SOURCE,
            todo_component_mask_root(look, TodoComponentRecipe::Grouped),
        );
        let (planned, patchbay) = configured(
            PATCHBAY_COMPONENT_MASK_SOURCE,
            patchbay_component_mask_root(look),
        );
        assert_eq!(patchbay.look, todo.look);
        assert_ne!(patchbay.recipe, todo.recipe);
        assert_ne!(
            patchbay.component.source_document_id,
            todo.component.source_document_id
        );
        assert_eq!(
            configured_mask_configuration(&host(), &planned).unwrap(),
            Some(patchbay)
        );
        assert!(verify_plan(&planned.plan));
        let mut forged = planned.clone();
        forged.plan.fragments[0].placements[0].configuration.clear();
        assert!(configured_mask_configuration(&host(), &forged).is_err());
        let mut wrong_root = planned;
        wrong_root.mask.plot_name =
            todo_component_mask_root(look, TodoComponentRecipe::Grouped).into();
        assert!(configured_mask_configuration(&host(), &wrong_root).is_err());
    }
}
#[test]
fn registered_nonvisual_source_recovers_default_without_trusting_unknown_roots() {
    let (planned, configuration) =
        configured(PATCHBAY_COMPONENT_MASK_SOURCE, PATCHBAY_DEFAULT_MASK_ROOT);
    let admitted = admit_visual_look(
        &configuration.requested_look().unwrap(),
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    assert!(matches!(
        admitted.requested.visual(),
        LookVisualFacetOption::None
    ));
    assert_eq!(admitted.selected.identity(), PHOSPHOR_LOOK_ID);
    assert!(matches!(
        admitted.visual.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::MissingVisualFacet
        }
    ));
    assert_eq!(
        configured_mask_configuration(&host(), &planned).unwrap(),
        Some(configuration)
    );
    assert!(super::for_root("patchbay-mask-default@2").is_none());
    assert!(!super::contains(
        &(String::from(PATCHBAY_COMPONENT_MASK_SOURCE) + "\n")
    ));
    let mut absent = host();
    absent.capabilities.clear();
    assert!(configured_mask_configuration(&absent, &planned).is_err());
}

#[test]
fn field_presets_retain_authored_values_and_existing_recipes_through_revalidation() {
    use conduit_plot::rust_binding::NativeRustBinding;
    for (source, roots, original) in [
        (
            FIELD_INSTRUMENT_TODO_MASK_SOURCE,
            FIELD_INSTRUMENT_TODO_MASK_ROOTS.as_slice(),
            TODO_COMPONENT_MASK_SOURCE,
        ),
        (
            FIELD_INSTRUMENT_PATCHBAY_MASK_SOURCE,
            FIELD_INSTRUMENT_PATCHBAY_MASK_ROOTS.as_slice(),
            PATCHBAY_COMPONENT_MASK_SOURCE,
        ),
    ] {
        assert!(super::contains(source));
        let mut source_id = None;
        for root in roots {
            assert_eq!(super::for_root(root), Some(source));
            let (planned, configuration) = configured(source, root);
            let expected = if root.ends_with("-day") {
                field_instrument_day_look().unwrap()
            } else {
                field_instrument_night_look().unwrap()
            };
            assert_eq!(
                configuration.look.canonical_value(),
                expected.encode().unwrap()
            );
            let original_root = root
                .replace("field-instrument-day", "compact")
                .replace("field-instrument-night", "compact");
            let (_, baseline) = configured(original, &original_root);
            assert_eq!(configuration.recipe, baseline.recipe);
            assert_eq!(
                configuration.component.checked_plot_id,
                baseline.component.checked_plot_id
            );
            assert_eq!(configuration.component.fore_id, baseline.component.fore_id);
            assert_eq!(
                configured_mask_configuration(&host(), &planned).unwrap(),
                Some(configuration.clone())
            );
            if root.ends_with("-day") {
                let mut forged = planned.clone();
                forged.mask.plot_name = root.replace("-day", "-night");
                assert!(configured_mask_configuration(&host(), &forged).is_err());
            }
            if let Some(previous) = &source_id {
                assert_eq!(&configuration.component.source_document_id, previous);
            }
            source_id = Some(configuration.component.source_document_id);
        }
        assert!(!super::contains(&(String::from(source) + "\n")));
    }
    assert!(super::for_root("todo-mask-flat-field-instrument-day@2").is_none());
}
