//! Local proposal resolution, not rendering, Host selection or acknowledged Show proof.
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn resolve(
    author: &LookInfo,
    local: &LocalLookInfo,
    user: VisualUserConstraints,
) -> Result<ResolvedLocalLook, LocalLookRefusal> {
    resolve_local_visual_look(author, local, user, LocalLookContrast::Standard)
}
fn user() -> VisualUserConstraints {
    VisualUserConstraints {
        forced_colors: false,
        reduced_motion: false,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 200,
    }
}
fn local(values: Vec<LocalLookVisualOverride>) -> LocalLookInfo {
    LocalLookInfo::new(
        LocalLookContrast::Standard,
        "personal/readable@1".into(),
        1,
        BoundedSequence::try_from_iter(values).unwrap(),
    )
    .unwrap()
}
fn tokens(look: &LookInfo) -> &LookVisualTokensInfo {
    let LookVisualFacetOption::Some(visual) = look.visual() else {
        panic!("visual")
    };
    visual.tokens()
}
#[test]
fn partial_values_preserve_every_unset_token_and_nonvisual_facet() {
    let author = phosphor_look().unwrap();
    let original = author.clone();
    let local = local(vec![
        LocalLookVisualOverride::TypeBodyPx(18),
        LocalLookVisualOverride::SpacePanelPx(0),
    ]);
    let resolved = resolve(&author, &local, user()).unwrap();
    let selected = &resolved.admitted.selected;
    assert_eq!(*tokens(selected).type_body_px(), 18);
    assert_eq!(*tokens(selected).space_panel_px(), 0);
    assert_eq!(
        tokens(selected).text_secondary(),
        tokens(&author).text_secondary()
    );
    assert_eq!(
        tokens(selected).type_small_px(),
        tokens(&author).type_small_px()
    );
    let before = tokens(&author).clone().into_structured().unwrap();
    let after = tokens(selected).clone().into_structured().unwrap();
    let conduit_core::StructuredInfoValueShape::Record(before) = before.shape() else {
        panic!("token record")
    };
    let conduit_core::StructuredInfoValueShape::Record(after) = after.shape() else {
        panic!("token record")
    };
    assert_eq!(before.len(), after.len());
    for field in before {
        let retained = after
            .iter()
            .find(|candidate| candidate.name() == field.name())
            .unwrap();
        if !matches!(field.name(), "type_body_px" | "space_panel_px") {
            assert_eq!(
                field.value(),
                retained.value(),
                "unset field {}",
                field.name()
            );
        }
    }
    assert_eq!(selected.spoken(), author.spoken());
    assert_eq!(selected.generative(), author.generative());
    assert_eq!(
        resolved.field_origin("type_body_px"),
        Some(LocalLookValueOrigin::Local)
    );
    assert_eq!(
        resolved.field_origin("type_small_px"),
        Some(LocalLookValueOrigin::Author)
    );
    assert_eq!(resolved.field_origin("#todo .delete"), None);
    assert_ne!(selected.identity(), author.identity());
    let bytes = selected.clone().encode().unwrap();
    assert!(bytes.len() <= MAX_LOOK_BYTES);
    assert_eq!(LookInfo::decode(&bytes).unwrap(), *selected);
    assert_eq!(author, original);
    assert_eq!(resolved.author, original);
    assert_eq!(resolved.local, local);
    assert_eq!(
        resolved.admitted,
        resolve(&author, &local, user()).unwrap().admitted
    );
}
#[test]
fn empty_override_preserves_author_identity_and_duplicates_do_not_gain_order_precedence() {
    let author = engineering_look().unwrap();
    assert_eq!(
        resolve(&author, &local(vec![]), user())
            .unwrap()
            .admitted
            .selected,
        author
    );
    for values in [
        vec![
            LocalLookVisualOverride::SpacePanelPx(0),
            LocalLookVisualOverride::SpacePanelPx(12),
        ],
        vec![
            LocalLookVisualOverride::SpacePanelPx(12),
            LocalLookVisualOverride::SpacePanelPx(0),
        ],
    ] {
        assert!(matches!(
            resolve(&author, &local(values), user()),
            Err(LocalLookRefusal::DuplicateField("space_panel_px"))
        ));
    }
}
#[test]
fn source_ranges_and_resource_cardinality_remain_checked() {
    for value in [
        LocalLookVisualOverride::TypeBodyPx(7),
        LocalLookVisualOverride::TypeSmallPx(65),
        LocalLookVisualOverride::TextScalePercent(99),
        LocalLookVisualOverride::FocusWidthPx(0),
    ] {
        assert!(LocalLookInfo::new(
            LocalLookContrast::Standard,
            "local/range".into(),
            1,
            BoundedSequence::try_from_iter(vec![value]).unwrap()
        )
        .is_err());
    }
    assert!(
        BoundedSequence::<LocalLookVisualOverride, 29>::try_from_iter(
            vec![LocalLookVisualOverride::SpacePanelPx(0); 30]
        )
        .is_err()
    );
}
#[test]
fn accessibility_and_host_fallback_keep_their_origins_and_original_requests() {
    let author = phosphor_look().unwrap();
    let local = local(vec![
        LocalLookVisualOverride::TextScalePercent(200),
        LocalLookVisualOverride::Motion(VisualMotionPreference::Restrained),
    ]);
    let resolved = resolve(
        &author,
        &local,
        VisualUserConstraints {
            reduced_motion: true,
            minimum_text_scale_percent: 125,
            maximum_supported_text_scale_percent: 150,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(resolved.admitted.visual.expression.text_scale_percent, 125);
    assert!(matches!(
        resolved.admitted.visual.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::HostTextCapacity
        }
    ));
    assert_eq!(
        resolved.field_origin("text_scale_percent"),
        Some(LocalLookValueOrigin::Accessibility)
    );
    assert_eq!(
        resolved.field_origin("background"),
        Some(LocalLookValueOrigin::Fallback)
    );
    assert_eq!(resolved.author, author);
    assert_eq!(resolved.local, local);
    assert!(resolve(
        &author,
        &local,
        VisualUserConstraints {
            minimum_text_scale_percent: 200,
            maximum_supported_text_scale_percent: 125,
            ..user()
        }
    )
    .is_err());
}
#[test]
fn contrast_checks_resolved_pairs_and_identifies_the_originating_layer() {
    let author = editorial_look().unwrap();
    let bad = local(vec![
        LocalLookVisualOverride::text_primary(231, 240, 244).unwrap()
    ]);
    match resolve(&author, &bad, user()) {
        Err(LocalLookRefusal::Contrast {
            pair,
            foreground,
            background,
            ..
        }) => {
            assert_eq!(pair.foreground, "text_primary");
            assert_eq!(pair.background, "background");
            assert_eq!(foreground, LocalLookValueOrigin::Local);
            assert_eq!(background, LocalLookValueOrigin::Author);
            assert!(pair.lower_ratio_milli < 4500);
        }
        other => panic!("{other:?}"),
    }
    let forced = resolve(
        &author,
        &bad,
        VisualUserConstraints {
            forced_colors: true,
            ..user()
        },
    )
    .unwrap();
    assert_eq!(
        forced.contrast,
        LocalLookContrastOutcome::PlatformColorsRequired
    );
    assert_eq!(
        forced.field_origin("text_primary"),
        Some(LocalLookValueOrigin::Platform)
    );
    // Forced platform colors still require the selected Host's real color proof.
    assert!(forced.admitted.visual.platform_forced_colors);
}
#[test]
fn all_existing_builtin_palettes_pass_standard_nominal_pair_checks() {
    for author in [
        phosphor_look().unwrap(),
        editorial_look().unwrap(),
        engineering_look().unwrap(),
        navy_look(VisualDensity::Compact, VisualTypography::Editorial).unwrap(),
    ] {
        assert_eq!(
            resolve(&author, &local(vec![]), user()).unwrap().contrast,
            LocalLookContrastOutcome::CheckedNominalTokens
        );
    }
}
#[test]
fn enhanced_text_restraint_is_not_downgraded_by_fallback_or_local_omission() {
    let author = editorial_look().unwrap();
    let enhanced = LocalLookInfo::new(
        LocalLookContrast::Enhanced,
        "personal/strong".into(),
        1,
        BoundedSequence::new(),
    )
    .unwrap();
    assert!(matches!(
        resolve(&author, &enhanced, user()),
        Err(LocalLookRefusal::Contrast { .. })
    ));
    let values = vec![
        LocalLookVisualOverride::text_primary(0, 0, 0).unwrap(),
        LocalLookVisualOverride::text_secondary(0, 0, 0).unwrap(),
        LocalLookVisualOverride::emphasis(0, 0, 0).unwrap(),
        LocalLookVisualOverride::warning(0, 0, 0).unwrap(),
        LocalLookVisualOverride::failure(0, 0, 0).unwrap(),
        LocalLookVisualOverride::success(0, 0, 0).unwrap(),
    ];
    let enhanced = LocalLookInfo::new(
        LocalLookContrast::Enhanced,
        "personal/strong".into(),
        1,
        BoundedSequence::try_from_iter(values).unwrap(),
    )
    .unwrap();
    assert_eq!(
        resolve(&author, &enhanced, user()).unwrap().contrast,
        LocalLookContrastOutcome::CheckedNominalTokens
    );
}

#[test]
fn ordinary_conduitese_local_proposal_preserves_typed_ranges_and_closed_fields() {
    use conduit_plot::{
        check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
    };
    let mut catalog = StartupCatalog::new();
    for (name, ty) in [
        ("ThemeColor", ThemeColor::semantic_type().unwrap()),
        (
            "VisualDensityPreference",
            VisualDensityPreference::semantic_type().unwrap(),
        ),
        (
            "VisualTypographyPreference",
            VisualTypographyPreference::semantic_type().unwrap(),
        ),
        (
            "VisualMotionPreference",
            VisualMotionPreference::semantic_type().unwrap(),
        ),
    ] {
        catalog.insert_structured_type(name, ty).unwrap();
    }
    let text = include_str!("../examples/local-readable.conduit");
    let check = |text: &str| {
        check_syntax_document(
            &parse_syntax_document(&[LOCAL_LOOK_SOURCE, text].join("\n")),
            &catalog,
        )
    };
    let checked = check(text).unwrap();
    let Some(CanonicalStartupValue::Structured(value)) =
        &checked.plots[0].startup_parameters[0].default
    else {
        panic!("ordinary typed startup value")
    };
    let value = value.try_concrete().unwrap();
    let bound = LocalLookInfo::from_structured(value).unwrap();
    assert_eq!(
        bound,
        local(vec![
            LocalLookVisualOverride::TypeBodyPx(18),
            LocalLookVisualOverride::SpacePanelPx(0)
        ])
    );
    assert!(check(&text.replace("type_body_px(18)", "type_body_px(7)")).is_err());
    assert!(check(&text.replace("type_body_px(18)", "selector(\"#todo\")")).is_err());
    let canonical = bound
        .clone()
        .into_structured()
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(canonical.len() <= MAX_LOOK_BYTES);
}

#[test]
fn local_visual_values_cannot_manufacture_an_authors_absent_visual_facet() {
    let author = LookInfo::new(
        LookGenerativeFacetOption::None,
        "spoken/author".into(),
        LookSpokenFacetOption::some(LookSpokenDelivery::Calm, LookVerbosity::Brief).unwrap(),
        2,
        LookVisualFacetOption::None,
    )
    .unwrap();
    assert!(matches!(
        resolve(
            &author,
            &local(vec![LocalLookVisualOverride::TypeBodyPx(18)]),
            user()
        ),
        Err(LocalLookRefusal::AuthorHasNoVisualFacet)
    ));
    let untouched = resolve(&author, &local(vec![]), user()).unwrap();
    assert_eq!(untouched.author, author);
    assert_eq!(untouched.admitted.requested, author);
    assert!(matches!(
        untouched.admitted.visual.outcome,
        VisualExpressionOutcome::PhosphorFallback {
            refusal: VisualExpressionRefusal::MissingVisualFacet
        }
    ));
    assert_eq!(
        untouched.field_origin("background"),
        Some(LocalLookValueOrigin::Fallback)
    );
    assert_eq!(author.visual(), &LookVisualFacetOption::None);
}

#[test]
fn personal_proposal_cannot_weaken_an_independent_accessibility_contrast_floor() {
    let author = editorial_look().unwrap();
    let personal = local(vec![]);
    assert!(
        resolve_local_visual_look(&author, &personal, user(), LocalLookContrast::Enhanced).is_err()
    );
    let forced = resolve_local_visual_look(
        &author,
        &personal,
        VisualUserConstraints {
            forced_colors: true,
            ..user()
        },
        LocalLookContrast::Enhanced,
    )
    .unwrap();
    assert_eq!(forced.required_contrast, LocalLookContrast::Enhanced);
    assert_eq!(
        forced.contrast_requirement_origin,
        LocalLookValueOrigin::Accessibility
    );
    assert_eq!(
        forced.contrast,
        LocalLookContrastOutcome::PlatformColorsRequired
    );
    assert_eq!(forced.local.contrast(), &LocalLookContrast::Standard);
}
