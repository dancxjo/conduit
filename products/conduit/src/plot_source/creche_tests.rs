//! Actual installed product Source catalogue, distinct from a runtime offer.
#[test]
fn shipped_creche_is_an_ordinary_checked_product_plot() {
    let parsed = super::parse(include_str!("../../../../plots/creche/main.conduit")).unwrap();
    let authoring = parsed.expand_entry_for_authoring().unwrap();
    assert_eq!(authoring.expanded.gears.len(), 1);
    assert_eq!(
        authoring.expanded.gears[0].kind_id.as_str(),
        conduit_body_make::deployment_runtime::CRECHE_KIND
    );
    assert_eq!(authoring.front.inputs().len(), 1);
    assert_eq!(authoring.front.outputs().len(), 1);
    assert_eq!(
        authoring.front.inputs()[0].value_kind.as_str(),
        conduit_body_make::deployment_runtime::CRECHE_REQUEST_KIND
    );
    assert_eq!(
        authoring.front.outputs()[0].value_kind.as_str(),
        conduit_body_make::deployment_runtime::CRECHE_OUTCOME_KIND
    );
}
