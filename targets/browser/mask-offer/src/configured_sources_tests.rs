//! Exact registered document/Look custody; no live owner or Show is claimed.
use crate::*;
use conduit_core::*;
use conduit_face::*;

fn host() -> HostAdvertisement {
    HostAdvertisement {
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
fn history_root_retains_checked_recipe_exact_required_slot_and_existing_look() {
    use conduit_plot::rust_binding::NativeRustBinding;
    let (planned, configuration) =
        configured(HISTORY_COMPONENT_MASK_SOURCE, HISTORY_COMPONENT_MASK_ROOT);
    let (_, editorial) = configured(
        TODO_COMPONENT_MASK_SOURCE,
        todo_component_mask_root(
            ComponentMaskLook::CompactEditorial,
            TodoComponentRecipe::Flat,
        ),
    );
    assert_eq!(configuration.look, editorial.look);
    assert_eq!(
        configured_mask_configuration(&host(), &planned).unwrap(),
        Some(configuration.clone())
    );
    assert_eq!(
        super::for_root(HISTORY_COMPONENT_MASK_ROOT),
        Some(HISTORY_COMPONENT_MASK_SOURCE)
    );
    assert!(!super::contains(
        &(String::from(HISTORY_COMPONENT_MASK_SOURCE) + "\n")
    ));
    let decode = || {
        StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value()).unwrap()
    };
    let recipe = ComponentRecipeInfo::from_structured(decode()).unwrap();
    assert_eq!(recipe.identity(), HISTORY_COMPONENT_RECIPE_ID);
    assert_eq!(recipe.regions().len(), 1);
    let ComponentSubjectSelector::Bound(binding) = recipe.regions()[0].selector() else {
        panic!("history selection must be an exact required slot")
    };
    assert_eq!(binding.subject(), "environment-history");
    assert!(matches!(binding.role(), ComponentBindingRole::Role(role) if role == "Collection"));
    assert_eq!(binding.properties().len(), 8);
    assert!(binding.actions().is_empty());
    let face = history_slot_face("environment-history", FaceRole::Collection, false, false);
    let prepared = prepare_component_recipe(decode(), &face).unwrap();
    assert_eq!(prepared.subject_regions, vec![Some(0)]);
    for malformed in [
        history_slot_face("other-history", FaceRole::Collection, false, false),
        history_slot_face("environment-history", FaceRole::Info, false, false),
        history_slot_face("environment-history", FaceRole::Collection, true, false),
        history_slot_face("environment-history", FaceRole::Collection, false, true),
    ] {
        assert!(prepare_component_recipe(decode(), &malformed).is_err());
    }
    let mut wrong_root = planned;
    wrong_root.mask.plot_name = "environment-history-mask-unreviewed".into();
    assert!(configured_mask_configuration(&host(), &wrong_root).is_err());
}

fn history_slot_face(
    subject: &str,
    role: FaceRole,
    wrong_clock: bool,
    missing_producer: bool,
) -> Face {
    let mut properties = vec![];
    for (name, value) in [
        (
            "schema",
            FacePropertyValue::Text("environment/history@1".into()),
        ),
        (
            "clock",
            if wrong_clock {
                FacePropertyValue::Count(0)
            } else {
                FacePropertyValue::Text("producer-boot-ms".into())
            },
        ),
        (
            "producer-host",
            FacePropertyValue::Identity("host/history-fixture".into()),
        ),
        (
            "producer-boot",
            FacePropertyValue::Identity("boot/history-fixture".into()),
        ),
        (
            "producer-plan",
            FacePropertyValue::Identity("plan/history-fixture".into()),
        ),
        (
            "producer-play",
            FacePropertyValue::Identity("play/history-fixture".into()),
        ),
        ("visible-samples", FacePropertyValue::Count(0)),
        ("samples-outside-view", FacePropertyValue::Count(0)),
    ] {
        if missing_producer && name == "producer-plan" {
            continue;
        }
        properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: subject.into(),
            role,
            name: "History slot fixture".into(),
        }],
        vec![],
        properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
