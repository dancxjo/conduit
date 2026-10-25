use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

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
        LookQuantityFacetOption::None,
        LookSpokenFacetOption::None,
        3,
        LookVisualFacetOption::None,
    )
    .unwrap();
    assert_eq!(
        look_facet_availability(&absent, &LookFacetRequest::Visual, false),
        LookFacetAvailability::Absent
    );
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
        LookQuantityFacetOption::None,
        original.spoken().clone(),
        3,
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
        LookQuantityFacetOption::None,
        original.spoken().clone(),
        3,
        original.visual().clone(),
    )
    .is_err());
}

#[test]
fn absent_visual_policy_uses_default_without_losing_other_requested_facets() {
    let original = phosphor_look().unwrap();
    let requested = LookInfo::new(
        original.generative().clone(),
        "conduit.look/nonvisual@1".into(),
        LookQuantityFacetOption::None,
        original.spoken().clone(),
        3,
        LookVisualFacetOption::None,
    )
    .unwrap();
    let before = requested.clone();
    for constraints in [
        user(),
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            ..user()
        },
    ] {
        let selected = admit_visual_look(&requested, constraints).unwrap();
        assert_eq!(requested, before);
        assert_eq!(selected.requested, before);
        assert_eq!(selected.selected, original);
        assert_eq!(
            selected.visual.expression.text_scale_percent,
            constraints.minimum_text_scale_percent
        );
        assert_eq!(
            selected.visual.platform_forced_colors,
            constraints.forced_colors
        );
        assert!(matches!(
            selected.visual.outcome,
            VisualExpressionOutcome::PhosphorFallback {
                refusal: VisualExpressionRefusal::MissingVisualFacet
            }
        ));
        // Synthetic admission output for a real DOM-renderer capture. It is
        // not a browser Mask Source/Plan, Show, or Body execution receipt.
        let wire = |look: &LookInfo| {
            let structured = look.clone().into_structured().unwrap();
            conduit_core::StructuredConfigurationValue::new(
                structured
                    .value_type()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
                structured.canonical_bytes().unwrap(),
            )
            .unwrap()
        };
        eprintln!(
            "NONVISUAL_ADMISSION_{}={}",
            if constraints.forced_colors {
                "FORCED"
            } else {
                "NORMAL"
            },
            serde_json::json!({
                "requested_look": wire(&selected.requested),
                "selected_look": wire(&selected.selected),
                "expression": selected.visual,
            })
        );
        for request in [LookFacetRequest::Spoken, LookFacetRequest::Generative] {
            assert_eq!(
                look_facet_availability(&selected.requested, &request, true),
                LookFacetAvailability::Selected
            );
        }
        assert_eq!(
            look_facet_availability(&selected.requested, &LookFacetRequest::Visual, true),
            LookFacetAvailability::Absent
        );
    }
    assert!(matches!(
        admit_visual_look(
            &requested,
            VisualUserConstraints {
                minimum_text_scale_percent: 200,
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
fn visual_fallback_and_accessibility_never_replace_quantity_unit_or_absence() {
    for quantity in [
        LookQuantityFacetOption::None,
        LookQuantityFacetOption::some(LookTemperatureUnit::Celsius).unwrap(),
        LookQuantityFacetOption::some(LookTemperatureUnit::Kelvin).unwrap(),
        LookQuantityFacetOption::some(LookTemperatureUnit::Fahrenheit).unwrap(),
    ] {
        for visual in [large_request(), phosphor_look().unwrap()] {
            for absent in [false, true] {
                let requested = LookInfo::new(
                    visual.generative().clone(),
                    visual.identity().clone(),
                    quantity.clone(),
                    visual.spoken().clone(),
                    3,
                    if absent {
                        LookVisualFacetOption::None
                    } else {
                        visual.visual().clone()
                    },
                )
                .unwrap();
                for constraints in [
                    user(),
                    VisualUserConstraints {
                        maximum_supported_text_scale_percent: 125,
                        ..user()
                    },
                    VisualUserConstraints {
                        forced_colors: true,
                        reduced_motion: true,
                        minimum_text_scale_percent: 150,
                        ..user()
                    },
                ] {
                    let result = admit_visual_look(&requested, constraints).unwrap();
                    assert_eq!(result.requested, requested);
                    assert_eq!(result.selected.quantity(), &quantity);
                    if absent {
                        // The explicit absent visual case always takes fallback;
                        // the remaining cases exercise ordinary/Host limit paths.
                        assert!(matches!(
                            result.visual.outcome,
                            VisualExpressionOutcome::PhosphorFallback { .. }
                        ));
                    }
                }
            }
        }
    }
}
