use super::prepare_pair_for;

#[test]
fn scoped_glyph_constants_prepare_in_one_explicit_two_host_plan() {
    let source =
        include_str!("../../../../../../proof/browser/fixtures/scoped-pattern-glyph.conduit");
    let _ = prepare_pair_for(source);
    assert!(super::super::plan::prepare(
        "browser/a",
        "boot/a",
        "browser/b",
        "boot/b",
        &source.replace("with text/pattern/notation as r\n", "")
    )
    .is_err());
}

#[test]
fn sealed_foreign_expression_capability_refuses_at_receiving_host() {
    let source =
        include_str!("../../../../../../proof/browser/fixtures/scoped-pattern-glyph.conduit");
    let mut plan =
        super::super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", source)
            .unwrap()
            .plan;
    let expression = plan
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .find(|placement| {
            placement.kind_contract_revision.as_str() == conduit_plot::PURE_EXPRESSION_REVISION
        })
        .unwrap();
    expression.capability_id = "foreign/expression-capability".into();
    let forged = conduit_core::seal_plan_with_realization_backs_and_completion(
        conduit_core::PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.fragments,
    );
    assert!(conduit_core::verify_plan(&forged));
    let error = super::super::plan::accept(forged, "browser/b", "boot/b")
        .err()
        .unwrap();
    assert!(
        error.contains("capability differs from its exact contract"),
        "{error}"
    );
}
