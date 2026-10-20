//! Qualified numerical meaning through the supported product entrance.
use super::*;
#[test]
fn absolute_frequency_fore_keeps_certified_source_and_error_in_one_product_play() {
    let plan = run(
        include_str!("../fixtures/numerical_fore_near.conduit"),
        "Near frequency",
        "certified",
    );
    let gears = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .collect::<Vec<_>>();
    assert!(gears.iter().any(|g| g.kind_id.as_str() == "math/near"
        && g.execution_profile_id.as_str() == "std/quantity-near-binary32-single-round@1"));
    let qualified = gears
        .iter()
        .find(|g| {
            g.semantic_contract.value_contracts().iter().any(|c| {
                c.contract.constraints.iter().any(|c| {
                    matches!(c, conduit_core::ValueConstraint::NearQuantity { source_kind, .. }
            if source_kind.as_str() == conduit_core::FREQUENCY_INFO_ID)
                })
            })
        })
        .expect("selected qualified consumer");
    assert!(qualified.realization_properties.iter().any(
        |p| p.profile() == &conduit_std_offers::pure_expression_value_contract_bounds_profile()
    ));
}
