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
    assert_eq!(
        qualified.execution_profile_id.as_str(),
        conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
    );
    let bounds = qualified
        .realization_properties
        .iter()
        .find(|p| p.profile() == &conduit_std_offers::near_contract_bounds_profile())
        .expect("selected Near consumer admits replay work and storage");
    let value =
        conduit_core::validate_canonical_structured_value(bounds.canonical_value()).unwrap();
    let read = |name| {
        let field = value.record_field(name).unwrap().unwrap();
        let node = field.value_node();
        assert_eq!(node[0], 0);
        assert_eq!(node.len(), 13);
        u64::from_le_bytes(node[5..].try_into().unwrap())
    };
    assert_eq!(read("maximum-constraints"), 1);
    assert_eq!(
        read("maximum-predicate-replay-work"),
        conduit_std_offers::NEAR_CONTRACT_REPLAY_WORK
    );
    assert!(read("prepared-replay-container-and-buffer-bytes") > 0);
    assert!(read("retained-near-schema-bytes") > 0);
    assert_eq!(read("maximum-validated-input-bytes"), 131_072);
}
