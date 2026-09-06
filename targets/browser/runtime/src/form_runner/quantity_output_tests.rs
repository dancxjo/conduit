//! The actual authored Forms reaching the browser's ordinary typed output effect.

use super::*;

fn pointer_quantity_source() -> String {
    format!(
        r#"{}
form zz-pointer-quantity {{
 pointer: input/pointer-source
 normalize: math/normalized-quantity-scalar
 map: quantity-range-map
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 pointer.pointer > project(PointerEvent.position) > project(Point2.x) > normalize.in
 normalize.out > map.control
 map.quantity > wrap.in
 wrap.out > show.input
}}"#,
        include_str!("../../../../../forms/quantity-range-map/main.conduit")
    )
}

#[test]
fn pointer_quantity_chain_uses_selectors_mapping_and_correlated_presentation() {
    let source = pointer_quantity_source();
    for (position_x, expected) in [(0, "20 Hz"), (500_000, "10010 Hz"), (1_000_000, "20000 Hz")] {
        let (mut session, effect) = TourSession::prepare_with_profile(
            "pointer-controller",
            "pointer-boot",
            &source,
            position_x as u64,
            MorseRealization::Direct,
            crate::installed_browser::PresentationProfile::Quantity,
        )
        .unwrap();
        let TourHostEffect::PointerEvent(input) = effect else {
            panic!("expected pointer acquisition");
        };
        let canonical = conduit_semantic_catalog::normalized_pointer_value(
            conduit_semantic_catalog::NormalizedPointerSample {
                position_x,
                position_y: 0,
                delta_x: 0,
                delta_y: 0,
                primary_pressed: false,
                coalesced: 0,
                dropped: 0,
                queue_capacity: 1,
                sequence: 1,
            },
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let TourProgress::Effect(output) = session.advance_with_output(&canonical).unwrap() else {
            panic!("pointer mapping must manifest");
        };
        let TourHostEffect::Manifestation(output) = *output else {
            panic!("expected mapped output");
        };
        assert_eq!(output.text.as_deref(), Some(expected));
        assert_eq!(input.active_play_id, output.active_play_id);
        assert_eq!(
            output
                .expanded_gears
                .iter()
                .filter(|gear| gear.kind_id.starts_with("structured-info/selector-"))
                .count(),
            2
        );
        let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
            panic!("expected completion");
        };
        assert_eq!(receipt.disposition, "completed");
        assert_eq!(receipt.active_play_id, output.active_play_id);
    }
}

#[test]
fn pointer_quantity_chain_preserves_incompatible_unit_failure_without_presentation() {
    use conduit_core::{Quantity, QuantityUnit};
    let (mut session, _) = TourSession::prepare_with_profile(
        "pointer-negative",
        "pointer-negative-boot",
        &pointer_quantity_source(),
        1,
        MorseRealization::Direct,
        crate::installed_browser::PresentationProfile::Quantity,
    )
    .unwrap();
    let mut canonical = conduit_semantic_catalog::normalized_pointer_value(
        conduit_semantic_catalog::NormalizedPointerSample {
            position_x: 123_456,
            position_y: 0,
            delta_x: 0,
            delta_y: 0,
            primary_pressed: false,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
            sequence: 1,
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let original = Quantity::new(123_456, QuantityUnit::Millionth).encode();
    let offsets: Vec<_> = canonical
        .windows(original.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == original).then_some(index))
        .collect();
    assert_eq!(offsets.len(), 1, "mutate only the exact selected x leaf");
    canonical[offsets[0]..offsets[0] + original.len()]
        .copy_from_slice(&Quantity::new(123_456, QuantityUnit::Hertz).encode());
    // The record is still structurally canonical. Unit refusal belongs to the
    // explicit converter, not to JS acquisition or a selector's field meaning.
    conduit_core::StructuredInfoValue::from_canonical_bytes(&canonical).unwrap();
    let error = session.advance_with_output(&canonical).unwrap_err();
    assert!(
        error.contains("OperationFailed(Failure { code: InvalidInput, detail: 12 })"),
        "{error}"
    );
}

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
                crate::installed_browser::PresentationProfile::Quantity,
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
