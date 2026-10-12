use conduit_presentation::*;

fn user() -> VisualUserConstraints {
    VisualUserConstraints {
        forced_colors: false,
        reduced_motion: false,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 200,
    }
}
#[test]
fn expression_has_no_slot_action_topology_or_input_authority() {
    let value = serde_json::to_value(VisualExpression::phosphor()).unwrap();
    for field in [
        "action_id",
        "arguments",
        "subjects",
        "relationships",
        "topology",
        "focus_order",
        "keyboard_binding",
        "aria_role",
        "name",
        "disclosure",
    ] {
        let mut forged = value.clone();
        forged
            .as_object_mut()
            .unwrap()
            .insert(field.into(), serde_json::json!("forged"));
        assert_eq!(
            decode_visual_expression(&serde_json::to_vec(&forged).unwrap()),
            Err(VisualExpressionRefusal::Encoding),
            "{field}"
        );
    }
}
#[test]
fn invalid_expression_uses_phosphor_and_user_requirements_remain_authoritative() {
    let admitted = admit_visual_expression(
        Err(VisualExpressionRefusal::Encoding),
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 150,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(
        admitted.expression.identity,
        CONDUIT_APPLICATION_THEME.identity
    );
    assert_eq!(admitted.expression.text_scale_percent, 150);
    assert!(admitted.platform_forced_colors);
    assert_eq!(admitted.expression.motion, VisualMotion::None);
    assert_eq!(
        admitted.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::Encoding
        }
    );
}
#[test]
fn density_palette_and_text_preferences_are_independently_admitted() {
    for density in [VisualDensity::Compact, VisualDensity::Spacious] {
        for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
            let request = VisualExpression {
                density,
                palette,
                motion: VisualMotion::Restrained,
                ..VisualExpression::phosphor()
            };
            let admitted = admit_visual_expression(
                Ok(request),
                VisualUserConstraints {
                    reduced_motion: true,
                    minimum_text_scale_percent: 150,
                    ..user()
                },
            )
            .unwrap();
            assert_eq!(admitted.expression.density, density);
            assert_eq!(admitted.expression.palette, palette);
            assert_eq!(admitted.expression.motion, VisualMotion::None);
            assert_eq!(admitted.expression.text_scale_percent, 150);
            assert_eq!(
                admitted.outcome,
                VisualExpressionOutcome::AccessibilityOverride {
                    forced_colors: false,
                    reduced_motion: true,
                    text_scale: true
                }
            );
        }
    }
}
#[test]
fn host_capacity_selects_fallback_without_discarding_user_text_requirement() {
    let request = VisualExpression {
        text_scale_percent: 200,
        ..VisualExpression::phosphor()
    };
    let admitted = admit_visual_expression(
        Ok(request.clone()),
        VisualUserConstraints {
            minimum_text_scale_percent: 125,
            maximum_supported_text_scale_percent: 150,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(admitted.expression.text_scale_percent, 125);
    assert_eq!(
        admitted.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::HostTextCapacity
        }
    );
    assert_eq!(
        admit_visual_expression(
            Ok(request),
            VisualUserConstraints {
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 150,
                ..user()
            }
        ),
        Err(VisualExpressionRefusal::HostTextCapacity)
    );
}

#[test]
fn duplicate_fields_and_capacity_edges_are_rejected_or_explicit() {
    let encoded = serde_json::to_string(&VisualExpression::phosphor()).unwrap();
    for duplicate in ["identity\":\"forged", "density\":\"spacious"] {
        let forged = format!("{{\"{duplicate}\",{}", &encoded[1..]);
        assert_eq!(
            decode_visual_expression(forged.as_bytes()),
            Err(VisualExpressionRefusal::Encoding)
        );
    }
    let mut request = VisualExpression::phosphor();
    request.identity = "x".repeat(MAX_VISUAL_EXPRESSION_ID_BYTES);
    assert!(request.validate().is_ok());
    request.identity.push('x');
    assert_eq!(request.validate(), Err(VisualExpressionRefusal::Identity));
    assert_eq!(
        decode_visual_expression(&vec![b' '; 1025]),
        Err(VisualExpressionRefusal::Encoding)
    );
    let admitted = admit_visual_expression(
        Err(VisualExpressionRefusal::Encoding),
        VisualUserConstraints {
            forced_colors: true,
            reduced_motion: true,
            minimum_text_scale_percent: 200,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(admitted.expression.text_scale_percent, 200);
    assert_eq!(
        admitted.accessibility_overrides,
        VisualAccessibilityOverrides {
            forced_colors: true,
            reduced_motion: true,
            text_scale: true
        }
    );
}

#[test]
fn structured_expression_uses_exact_checked_schema_and_validated_values() {
    for density in [VisualDensity::Compact, VisualDensity::Spacious] {
        let requested = VisualExpression {
            density,
            ..VisualExpression::phosphor()
        };
        let info = visual_expression_info(&requested).unwrap();
        assert_eq!(info.value_type(), &visual_expression_type());
        assert_eq!(visual_expression_from_info(info).unwrap(), requested);
    }
    let other = conduit_core::StructuredInfoValue::leaf(
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/text")).unwrap(),
        b"forged recipe and action slots".to_vec(),
    )
    .unwrap();
    assert_eq!(
        visual_expression_from_info(other),
        Err(VisualExpressionRefusal::Encoding)
    );
}
