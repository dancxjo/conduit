use conduit_face::*;

fn user() -> VisualUserConstraints {
    VisualUserConstraints {
        forced_colors: false,
        reduced_motion: false,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 200,
    }
}

#[test]
fn facet_absence_unknown_and_observed_host_unavailability_stay_distinct() {
    let look = phosphor_look().unwrap();
    for request in [
        LookFacetRequest::Visual,
        LookFacetRequest::Spoken,
        LookFacetRequest::Generative,
    ] {
        assert_eq!(
            look_facet_availability(&look, &request, true),
            LookFacetAvailability::Selected
        );
        assert_eq!(
            look_facet_availability(&look, &request, false),
            LookFacetAvailability::HostUnavailable
        );
    }
    let absent = LookInfo::new(
        LookGenerativeFacetOption::None,
        "conduit.look/plain@1".into(),
        LookSpokenFacetOption::None,
        2,
        LookVisualFacetOption::None,
    )
    .unwrap();
    assert_eq!(
        look_facet_availability(&absent, &LookFacetRequest::Visual, false),
        LookFacetAvailability::Absent
    );
    assert!(matches!(
        admit_visual_look(&absent, user()),
        Err(LookVisualAdmissionRefusal::Facet(
            LookFacetAvailability::Absent
        ))
    ));
    let unknown = LookFacetRequest::unsupported("haptic".into()).unwrap();
    assert_eq!(
        look_facet_availability(&look, &unknown, true),
        LookFacetAvailability::Unsupported
    );
}

#[test]
fn accessibility_changes_realization_without_rewriting_the_look_resource() {
    let look = phosphor_look().unwrap();
    let before = look.clone();
    let result = admit_visual_look(
        &look,
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 150,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(look, before);
    assert_eq!(result.requested, before);
    assert_eq!(result.selected, before);
    assert_eq!(result.visual.expression.text_scale_percent, 150);
    assert!(result.visual.platform_forced_colors);
    assert!(result.visual.accessibility_overrides.reduced_motion);
    assert!(matches!(
        result.visual.outcome,
        VisualExpressionOutcome::AccessibilityOverride { .. }
    ));
}

fn large_request() -> LookInfo {
    let original = phosphor_look().unwrap();
    let LookVisualFacetOption::Some(visual) = original.visual() else {
        unreachable!()
    };
    let identity = "conduit.look/large@1";
    let expression = VisualExpressionInfo::new(
        VisualDensityPreference::Spacious,
        identity.into(),
        VisualMotionPreference::Restrained,
        VisualPalettePreference::Phosphor,
        200,
        VisualTypographyPreference::Editorial,
        2,
    )
    .unwrap();
    LookInfo::new(
        original.generative().clone(),
        identity.into(),
        original.spoken().clone(),
        2,
        LookVisualFacetOption::some(expression, visual.tokens().clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn host_limit_fallback_retains_requested_and_selected_resource_truth() {
    let requested = large_request();
    let result = admit_visual_look(
        &requested,
        VisualUserConstraints {
            maximum_supported_text_scale_percent: 125,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(result.requested, requested);
    assert_eq!(result.selected.identity(), PHOSPHOR_LOOK_ID);
    assert!(matches!(
        result.visual.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::HostTextCapacity
        }
    ));
    assert!(matches!(
        admit_visual_look(
            &requested,
            VisualUserConstraints {
                minimum_text_scale_percent: 150,
                maximum_supported_text_scale_percent: 125,
                ..user()
            }
        ),
        Err(LookVisualAdmissionRefusal::Visual(
            VisualExpressionRefusal::HostTextCapacity
        ))
    ));
}

#[test]
fn foreign_visual_identity_cannot_be_attached_to_another_look() {
    let original = phosphor_look().unwrap();
    assert!(LookInfo::new(
        original.generative().clone(),
        "conduit.look/forged@1".into(),
        original.spoken().clone(),
        2,
        original.visual().clone()
    )
    .is_err());
}
