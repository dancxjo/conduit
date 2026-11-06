//! Checked nested Source, independent work facts and source-bound refusals.
use conduit_core::*;
use conduit_plot::{semantic_effects::*, *};

const SOURCE: &str = r#"plot inner (
 >> value: Scalar
 result: Scalar >>
) {
 work: fixture/changed()
 value >> work.value
 work.result >> result
}.
plot root (
 >> value: Scalar
 result: Scalar >>
) {
 first: fixture/pure()
 next: inner()
 value >> first.value
 first.result >> next.value
 next.result >> result
}.
"#;

fn kind(name: &str, laws: Vec<KindSemanticLaw>) -> Kind {
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!("{name}@1")),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port("value", PortDirection::Input)],
        outputs: vec![port("result", PortDirection::Output)],
        configuration: vec![],
        semantic_laws: laws,
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}
fn expanded(
    laws: Vec<KindSemanticLaw>,
) -> (ExpandedCanonicalPlot, CheckedSyntaxDocument, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for definition in [
        kind("fixture/pure", pure_expression_semantic_laws()),
        kind("fixture/changed", laws),
    ] {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(definition).unwrap();
    }
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    (
        expand_canonical_plot_for_authoring(&checked, "root", &profile)
            .unwrap()
            .expanded,
        checked,
        profile,
    )
}

#[test]
fn nested_checked_leaves_retain_all_axes_and_exact_source_origins() {
    let (graph, checked, profile) = expanded(pure_expression_semantic_laws());
    let inferred = graph
        .infer_semantic_effects(&checked, &profile, 64)
        .unwrap();
    assert!(core::ptr::eq(inferred.plot(), &graph));
    assert_eq!(inferred.leaves().len(), 2);
    assert!(inferred
        .leaves()
        .iter()
        .any(|leaf| leaf.origin().plot_path.len() > 1));
    let expected = kind("fixture/pure", pure_expression_semantic_laws());
    for leaf in inferred.leaves() {
        assert_eq!(
            leaf.facts().unwrap(),
            &semantic_work_facts(&expected).unwrap()
        );
        assert!(graph
            .gears
            .iter()
            .any(|gear| core::ptr::eq(gear, leaf.gear())));
        let span = leaf.origin().source_span;
        assert!(SOURCE[span.start..span.end].contains("fixture/"));
    }
    assert_eq!(
        inferred.summary().unwrap().replay,
        vec![ReplayBehavior::Exact]
    );
    inferred.require_pure_expression().unwrap();
}

#[test]
fn each_independent_impure_axis_reports_the_original_nested_source_span() {
    for (axis, replacement, expected) in [
        (
            0,
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
            PureExpressionRefusal::ExternalEffect,
        ),
        (
            1,
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            PureExpressionRefusal::TemporalState,
        ),
        (
            2,
            KindSemanticLaw::TimeDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::TimeDependence,
        ),
        (
            3,
            KindSemanticLaw::RandomDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::RandomDependence,
        ),
        (
            4,
            KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient),
            PureExpressionRefusal::ResourceDependence,
        ),
        (
            5,
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
            PureExpressionRefusal::Suspension,
        ),
        (
            6,
            KindSemanticLaw::Variability(VariabilityBehavior::AdmittedVariability),
            PureExpressionRefusal::Variability,
        ),
        (
            7,
            KindSemanticLaw::Replay(ReplayBehavior::Ineligible),
            PureExpressionRefusal::ReplayIneligible,
        ),
    ] {
        let mut laws = pure_expression_semantic_laws();
        laws[axis] = replacement;
        let (graph, checked, profile) = expanded(laws);
        let inferred = graph
            .infer_semantic_effects(&checked, &profile, 64)
            .unwrap();
        inferred.summary().unwrap(); // Impurity is a fact, not failed inference.
        let EffectInferenceRefusal::SemanticFact {
            gear,
            span,
            refusal,
        } = inferred.require_pure_expression().unwrap_err()
        else {
            panic!("source diagnostic")
        };
        assert_eq!(refusal, expected);
        assert!(gear.ends_with("work"));
        assert!(SOURCE[span.start..span.end].contains("fixture/changed"));
    }
}

#[test]
fn missing_and_duplicate_facts_remain_unknown_instead_of_pure() {
    let mut missing = pure_expression_semantic_laws();
    missing.remove(0);
    let mut duplicate = pure_expression_semantic_laws();
    duplicate.push(duplicate[0].clone());
    for (laws, expected) in [
        (
            missing,
            PureExpressionRefusal::MissingFact(PureExpressionFact::ExternalEffects),
        ),
        (
            duplicate,
            PureExpressionRefusal::DuplicateFact(PureExpressionFact::ExternalEffects),
        ),
    ] {
        let (graph, checked, profile) = expanded(laws);
        let inferred = graph
            .infer_semantic_effects(&checked, &profile, 64)
            .unwrap();
        assert_eq!(inferred.leaves().len(), 2);
        for result in [
            inferred.summary().map(|_| ()),
            inferred.require_pure_expression(),
        ] {
            assert!(
                matches!(result, Err(EffectInferenceRefusal::SemanticFact { refusal, .. }) if refusal == expected)
            );
        }
    }
}

#[test]
fn replay_contract_identity_is_retained_and_changed_graph_refuses() {
    let mut laws = pure_expression_semantic_laws();
    laws[7] = KindSemanticLaw::Replay(ReplayBehavior::Transactional {
        transaction_contract: kind_id("fixture/transaction@1"),
    });
    let (mut graph, checked, profile) = expanded(laws);
    let summary = graph
        .infer_semantic_effects(&checked, &profile, 64)
        .unwrap()
        .summary()
        .unwrap();
    assert!(summary.replay.contains(&ReplayBehavior::Exact));
    assert!(summary.replay.contains(&ReplayBehavior::Transactional {
        transaction_contract: kind_id("fixture/transaction@1")
    }));
    assert!(matches!(
        graph.infer_semantic_effects(&checked, &profile, 1),
        Err(EffectInferenceRefusal::Capacity)
    ));
    graph
        .gears
        .iter_mut()
        .find(|gear| gear.kind_id.as_str() == "fixture/changed")
        .unwrap()
        .semantic_contract
        .laws = pure_expression_semantic_laws();
    assert!(matches!(
        graph.infer_semantic_effects(&checked, &profile, 64),
        Err(EffectInferenceRefusal::ChangedCheckedExpansion)
    ));
}

#[test]
fn foreign_catalog_source_and_claimed_annotation_cannot_strengthen_facts() {
    let (graph, checked, profile) = expanded(pure_expression_semantic_laws());
    let mut laws = pure_expression_semantic_laws();
    laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    let (_, _, foreign) = expanded(laws);
    assert!(matches!(
        graph.infer_semantic_effects(&checked, &foreign, 64),
        Err(EffectInferenceRefusal::ChangedCheckedExpansion)
    ));
    let altered = check_syntax_document(
        &parse_syntax_document(&format!("# another Source\n{SOURCE}")),
        &{
            let mut startup = StartupCatalog::new();
            for name in ["fixture/pure", "fixture/changed"] {
                startup
                    .insert(KindSignature {
                        kind: name.into(),
                        startup_parameters: vec![],
                    })
                    .unwrap();
            }
            startup
        },
    )
    .unwrap();
    assert!(matches!(
        graph.infer_semantic_effects(&altered, &profile, 64),
        Err(EffectInferenceRefusal::ChangedCheckedExpansion)
    ));
    let mut startup = StartupCatalog::new();
    for name in ["fixture/pure", "fixture/changed"] {
        startup
            .insert(KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let claimed = SOURCE.replace("fixture/changed()", "fixture/changed(pure = true)");
    let refusal = check_syntax_document(&parse_syntax_document(&claimed), &startup).unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-022");
    assert_eq!(
        refusal.message,
        "unknown startup parameter 'pure'; '=' is declarative and there is no later assignment"
    );
    graph
        .infer_semantic_effects(&checked, &profile, 64)
        .unwrap()
        .require_pure_expression()
        .unwrap();
}

#[test]
fn deferred_activation_and_pool_bodies_are_explicit_gaps() {
    for (source, activation) in [
        ("plot identity (\n >> value: I32 <= 24B\n mapped: I32 <= 24B >>\n) = (.)\nplot root (\n >> values: I32...| <= 24B\n mapped: I32...| <= 24B >>\n) {\n child: activate(maximum-items = 2) identity()\n values >> child.value\n child.mapped >> mapped\n}\n", true),
        ("plot identity (\n >> value: I32 <= 24B\n mapped: I32 <= 24B >>\n) = (.)\nplot root {\n pool members: identity(size = 2)\n use: fixture/use-pool(members)\n}\n", false),
    ] {
        let mut startup = StartupCatalog::new();
        let mut catalog = ProfileCatalog::new();
        if !activation {
            startup.insert(KindSignature { kind: "fixture/use-pool".into(), startup_parameters: vec![StartupParameterSignature { name: "members".into(), value_type: "Pool".into(), default: None }] }).unwrap();
            catalog.insert(KindProjection { kind_id: kind_id("fixture/use-pool"), kind_contract_revision: KindIdentity::from("fixture/use-pool@1"), inputs: vec![], outputs: vec![], configuration: vec![] }).unwrap();
        }
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let authored = expand_canonical_plot_for_authoring(&checked, "root", &catalog).unwrap();
        let inferred = authored.expanded.infer_semantic_effects(&checked, &catalog, 64).unwrap();
        for result in [inferred.summary().map(|_| ()), inferred.require_pure_expression()] {
            assert!(if activation {
                matches!(result, Err(EffectInferenceRefusal::UnresolvedActivation { .. }))
            } else {
                matches!(result, Err(EffectInferenceRefusal::UnresolvedPool(_)))
            });
        }
    }
}
