use conduit_core::{
    ExactNumericWork, MathematicalValue, NumericalAlgorithm, ObservationKind,
    PreparedNumericalRealization, StructuredInfoValue, StructuredInfoValueShape,
};
const SOURCE: &str = include_str!("../../../plots/numerics/main.conduit");
#[test]
fn ordinary_product_source_retains_exact_meaning_through_selected_std_receipts() {
    let mut output = vec![];
    let execution = conduit::execute_hosted_plot(SOURCE, &mut output).unwrap();
    assert!(conduit_core::verify_plan(&execution.plan));
    let mut receipts = vec![];
    for observation in &execution.observations {
        if let ObservationKind::ValuePresented { value } = &observation.kind {
            assert!(observation.plan_id.is_some());
            assert!(observation.active_play_id.is_some());
            let receipt = StructuredInfoValue::from_canonical_bytes(&value.encoded).unwrap();
            assert_eq!(
                receipt.value_type(),
                &conduit_semantic_catalog::numerical_result_type()
            );
            let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
                panic!("receipt Type");
            };
            let field = fields.iter().find(|f| f.name() == "source").unwrap();
            let StructuredInfoValueShape::Leaf(encoded) = field.value().shape() else {
                panic!("source Type");
            };
            let source =
                MathematicalValue::decode(encoded, &mut ExactNumericWork::new(100_000_000))
                    .unwrap();
            assert_eq!(
                source,
                MathematicalValue::parse_literal("0.3", &mut ExactNumericWork::new(100_000_000))
                    .unwrap()
            );
            receipts.push(receipt);
        }
    }
    assert_eq!(receipts.len(), 2);
    let gears = execution
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .filter(|g| g.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND);
    for gear in gears {
        let certificate = PreparedNumericalRealization::from_property(
            &gear.realization_properties[0],
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        if gear.gear_id.as_str().ends_with("/exact") {
            assert_eq!(certificate.algorithm(), NumericalAlgorithm::ExactRational);
        } else {
            assert_eq!(certificate.algorithm(), NumericalAlgorithm::OutwardI64);
        }
        assert!(receipts
            .contains(&conduit_semantic_catalog::numerical_result_value(&certificate).unwrap()));
        assert!(execution
            .advertisements
            .iter()
            .flat_map(|h| &h.capabilities)
            .any(|offer| offer.capability_id == gear.capability_id));
    }
}
#[test]
fn symbolic_or_negative_precision_refuses_without_play_or_output() {
    for request in [
        "{value:pi,maximum-error:1}",
        "{value:0.3,maximum-error:pi}",
        "{value:0.3,maximum-error:-1}",
    ] {
        let source = format!("plot refuse {{\n value: math/realize(request={request})\n}}\n");
        let mut output = vec![];
        assert!(conduit::execute_hosted_plot(&source, &mut output).is_err());
        assert!(output.is_empty());
    }
}
