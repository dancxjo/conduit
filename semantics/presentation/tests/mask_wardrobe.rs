use conduit_core::{
    CheckedFormId, ExpandedFormId, FormIdentity, PlacementId, PlanId, SourceDocumentId,
};
use conduit_presentation::{
    MaskPlanningDisposition, MaskShowDisposition, MaskWardrobe, MaskWardrobeError,
    MaskWardrobeLifetime, SealedMaskFormRoute, SelectedMaskFormRoute,
};

fn mask(name: &str) -> FormIdentity {
    FormIdentity {
        source_document_id: SourceDocumentId::from(format!("source/{name}")),
        checked_form_id: CheckedFormId::from(format!("checked/{name}")),
        expanded_form_id: ExpandedFormId::from(format!("expanded/{name}")),
    }
}

fn route(mask_form: &FormIdentity, name: &str, available: bool) -> SealedMaskFormRoute {
    SealedMaskFormRoute {
        route_id: format!("route/{name}"),
        mask_form: mask_form.clone(),
        plan_id: PlanId::from("plan/current"),
        placement_ids: vec![PlacementId::from(format!("placement/{name}"))],
        currently_available: available,
    }
}

fn selected(mask_form: &FormIdentity, name: &str) -> SelectedMaskFormRoute {
    SelectedMaskFormRoute {
        route_id: format!("route/{name}"),
        mask_form: mask_form.clone(),
        plan_id: PlanId::from("plan/current"),
    }
}

#[test]
fn wear_doff_and_preference_are_revisioned_form_configuration() {
    let graphical = mask("graphical");
    let spoken = mask("spoken");
    let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![]).unwrap();
    let wardrobe = wardrobe.wear(0, graphical.clone()).unwrap();
    let wardrobe = wardrobe.wear(1, spoken.clone()).unwrap();
    let wardrobe = wardrobe.prefer(2, vec![spoken.clone()]).unwrap();
    assert_eq!(wardrobe.worn, vec![graphical.clone(), spoken.clone()]);
    assert_eq!(wardrobe.preference, vec![spoken.clone()]);
    let wardrobe = wardrobe.doff(3, &spoken).unwrap();
    assert_eq!(wardrobe.worn, vec![graphical]);
    assert!(wardrobe.preference.is_empty());
    assert_eq!(
        wardrobe.wear(3, mask("late")),
        Err(MaskWardrobeError::StaleRevision)
    );
}

#[test]
fn preference_does_not_disturb_a_valid_selected_mask_form() {
    let graphical = mask("graphical");
    let spoken = mask("spoken");
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![graphical.clone(), spoken.clone()],
        vec![spoken.clone()],
    )
    .unwrap();
    let routes = [
        route(&graphical, "graphical", true),
        route(&spoken, "spoken", true),
    ];
    let current = selected(&graphical, "graphical");
    let result = wardrobe
        .reconcile(&PlanId::from("plan/current"), &routes, Some(&current))
        .unwrap();
    assert_eq!(result.show, MaskShowDisposition::Retain(current));
    assert_eq!(result.planning, MaskPlanningDisposition::NotRequired);
}

#[test]
fn same_plan_fallback_uses_only_a_sealed_available_mask_form_route() {
    let spoken = mask("spoken");
    let wardrobe =
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![spoken.clone()], vec![]).unwrap();
    let routes = [
        route(&spoken, "generative", false),
        route(&spoken, "deterministic", true),
    ];
    let current = selected(&spoken, "generative");
    let result = wardrobe
        .reconcile(&PlanId::from("plan/current"), &routes, Some(&current))
        .unwrap();
    let MaskShowDisposition::SelectSealed { selected, .. } = result.show else {
        panic!("expected already-sealed fallback");
    };
    assert_eq!(selected.route_id, "route/deterministic");
    assert_eq!(selected.plan_id.as_str(), "plan/current");
    assert_eq!(result.planning, MaskPlanningDisposition::NotRequired);
}

#[test]
fn worn_but_unrealizable_form_has_no_show_and_requires_replacement_planning() {
    let spoken = mask("spoken");
    let wardrobe =
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![spoken.clone()], vec![]).unwrap();
    let result = wardrobe
        .reconcile(
            &PlanId::from("plan/current"),
            &[route(&spoken, "unavailable", false)],
            None,
        )
        .unwrap();
    assert!(matches!(
        result.show,
        MaskShowDisposition::NoCurrentShow { prior: None }
    ));
    assert_eq!(
        result.planning,
        MaskPlanningDisposition::ReplacementRequired
    );
}

#[test]
fn an_unsealed_or_other_plan_route_cannot_be_selected_as_fallback() {
    let spoken = mask("spoken");
    let wardrobe =
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![spoken.clone()], vec![]).unwrap();
    assert_eq!(
        wardrobe.reconcile(
            &PlanId::from("plan/current"),
            &[SealedMaskFormRoute {
                plan_id: PlanId::from("plan/replacement"),
                ..route(&spoken, "discovered", true)
            }],
            None,
        ),
        Err(MaskWardrobeError::InvalidRoute)
    );
}
