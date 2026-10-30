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
