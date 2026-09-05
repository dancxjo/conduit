//! The actual authored Forms reaching the browser's ordinary typed output effect.

use super::*;

#[test]
fn browser_quantity_authored_forms_reach_typed_output_and_completed_receipts() {
    for (authored, name, output, expected) in [
        (
            include_str!("../../../../../forms/quantity-range-map/main.conduit"),
            "quantity-range-map",
            "quantity",
            ["20 Hz", "10010 Hz", "20000 Hz"],
        ),
        (
            include_str!("../../../../../forms/normalized-light-intensity/main.conduit"),
            "normalized-light-intensity",
            "intensity",
            ["0 %", "50 %", "100 %"],
        ),
    ] {
        for (index, value) in [0, 500_000, 1_000_000].into_iter().enumerate() {
            let source = format!(
                r#"{authored}
form zz-quantity-output {{
 input: scalar/literal(value = {value})
 map: {name}
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 input.value > map.control
 map.{output} > wrap.in
 wrap.out > show.input
}}"#
            );
            let (mut session, effect) = TourSession::prepare_with_profile(
                "browser/quantity-output",
                "browser/quantity-output-boot",
                &source,
                index as u64,
                MorseRealization::Direct,
                true,
            )
            .unwrap();
            let TourHostEffect::Manifestation(effect) = effect else {
                panic!("expected an actual planned presentation");
            };
            assert_eq!(effect.text.as_deref(), Some(expected[index]));
            assert_eq!(
                effect.presentation_kind,
                conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND
            );
            assert_eq!(effect.plan_id, session.fragment.plan_id.as_str());
            assert_eq!(effect.active_play_id, session.active_play_id.as_str());
            assert!(!effect.presentation_id.is_empty());
            assert!(effect
                .expanded_gears
                .iter()
                .any(|gear| gear.kind_id == conduit_semantic_catalog::QUANTITY_MAP_KIND));
            let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
                panic!("one value must complete after manifestation");
            };
            assert_eq!(receipt.disposition, "completed");
            assert_eq!(receipt.active_play_id, effect.active_play_id);
        }
    }
}

#[test]
fn quantity_presenter_refuses_a_different_structured_leaf_type() {
    let value_type =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::SCALAR_INFO_ID))
            .unwrap();
    let value = conduit_core::StructuredInfoValue::leaf(
        value_type,
        conduit_core::Scalar::ONE.encode().to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(crate::installed_browser::decode_quantity_leaf(&value).is_err());
}
