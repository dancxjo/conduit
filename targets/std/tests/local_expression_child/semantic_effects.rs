//! Actual authored nested Source → ordinary Plan → existing local child Play.
use super::*;

const SOURCE: &str = include_str!("../fixtures/derived_effects.conduit");
const ENTRY: &str = "ordinary-effects-proof";

#[test]
fn inferred_nested_pure_path_executes_the_exact_plan_without_host_effects() {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let catalog = ProfileCatalog::new();
    let authored = expand_canonical_plot_for_authoring(&checked, ENTRY, &catalog).unwrap();
    let effects = authored
        .expanded
        .infer_semantic_effects(&checked, &catalog, 64)
        .unwrap();
    effects.require_pure_expression().unwrap();
    let leaf = &effects.leaves()[0];
    assert!(leaf.origin().plot_path.len() >= 3);
    let facts = effects.summary().unwrap();
    assert_eq!(facts.external_effects, [ExternalEffectBehavior::None]);
    assert_eq!(facts.temporal_state, [TemporalStateBehavior::None]);
    assert_eq!(facts.time_dependence, [SemanticDependence::None]);
    assert_eq!(facts.random_dependence, [SemanticDependence::None]);
    assert_eq!(facts.resource_dependence, [SemanticDependence::None]);
    assert_eq!(facts.suspension, [SuspensionBehavior::Never]);
    assert_eq!(
        facts.variability,
        [VariabilityBehavior::DeterministicFromInputs]
    );
    assert_eq!(facts.replay, [ReplayBehavior::Exact]);

    let (_, plan, offer) = planned_source(SOURCE, ENTRY);
    assert_eq!(plan.source_document_id, effects.plot().source_document_id);
    assert_eq!(plan.checked_plot_id, effects.plot().checked_plot_id);
    assert_eq!(plan.expanded_plot_id, effects.plot().expanded_plot_id);
    let placement = &plan.fragments[0].placements[0];
    assert_eq!(placement.semantic_contract, leaf.gear().semantic_contract);
    assert_eq!(
        placement.kind_contract_revision,
        leaf.gear().kind_contract_revision
    );
    assert_eq!(
        LocalExpressionFactory
            .budget(placement)
            .unwrap()
            .host_requests,
        0
    );
    let mut registry = KernelOperationRegistry::new();
    registry.install(LocalExpressionFactory).unwrap();
    let definition = definition(plan.clone(), offer);
    let input_port = definition.boundary.input_fronts[0].external_port.clone();
    let output_port = definition.boundary.output_fronts[0].external_port.clone();
    let mut host = KernelCompositeHost::prepare(definition, &registry).unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: 41_i32.to_le_bytes().to_vec(),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(4),
    };
    measured(|| {
        host.start().unwrap();
        host.admit_input(&input_port.port_id, 0, &input).unwrap();
        host.close_input(&input_port.port_id).unwrap();
        let mut deliveries = 0;
        for _ in 0..16 {
            host.step().unwrap();
            assert!(host.next_host_request().is_none());
            if let Some(sequence) = host.output_into(&output_port.port_id, &mut output).unwrap() {
                assert_eq!(output.encoded, 42_i32.to_le_bytes());
                host.complete_output(&output_port.port_id, sequence)
                    .unwrap();
                deliveries += 1;
            }
        }
        assert_eq!(deliveries, 1);
    });
    let signs = host.signs();
    assert!(!signs.is_empty());
    assert!(signs.values().flatten().all(|event| !matches!(
        event.kind,
        KernelEventKind::HostCallRequested | KernelEventKind::HostCallCompleted
    )));
    assert!(verify_plan(&plan));
}

#[test]
fn incompatible_back_effect_contract_refuses_before_any_play_or_output() {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let catalog = ProfileCatalog::new();
    let authored = expand_canonical_plot_for_authoring(&checked, ENTRY, &catalog).unwrap();
    let effects = authored
        .expanded
        .infer_semantic_effects(&checked, &catalog, 64)
        .unwrap();
    effects.require_pure_expression().unwrap();
    let (_, original, offer) = planned_source(SOURCE, ENTRY);
    let mut host = StdHost::new().advertisement().clone();
    host.capabilities = vec![offer];
    // The same Host/offer is eligible immediately before the sole mutation.
    conduit_planner::default_expanded_placements(&authored.expanded, std::slice::from_ref(&host))
        .unwrap();
    let law = host.capabilities[0]
        .semantic_contract
        .laws
        .iter_mut()
        .find(|law| matches!(law, KindSemanticLaw::ExternalEffects(_)))
        .unwrap();
    *law = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    assert!(!effects.leaves()[0]
        .gear()
        .accepts_realization(&host.capabilities[0]));
    assert!(conduit_planner::default_expanded_placements(&authored.expanded, &[host]).is_err());
    // There is no replacement Plan to start. Inspecting/rejecting this offer
    // does not mutate the retained original Plan or invoke a Back.
    assert!(verify_plan(&original));
    assert_eq!(
        effects.summary().unwrap().external_effects,
        [ExternalEffectBehavior::None]
    );
}
