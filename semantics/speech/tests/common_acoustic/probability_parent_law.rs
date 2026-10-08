use conduit_core::{
    StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::*;
pub fn forged_candidates() -> Vec<StructuredInfoValue> {
    let Ok(known) = SpeechProbabilitySpecification::known(1, 2) else {
        return Vec::new();
    };
    let known = known.into_structured().unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = known.shape() else {
        unreachable!()
    };
    let invalid = payload.clone();
    // The flattened Known payload exists structurally, but cannot be extracted as
    // an admitted UnitInterval. Variable/Gradient explicitly call that owner.
    assert!(SpeechUnitInterval::from_structured(invalid.clone()).is_err());
    let ty = SpeechProbabilitySpecification::semantic_type().unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        unreachable!()
    };
    let variable_ty = cases
        .iter()
        .find(|c| c.tag() == "variable")
        .unwrap()
        .payload_type()
        .clone();
    let variable = StructuredInfoValue::variant(
        ty.clone(),
        "variable",
        StructuredInfoValue::sequence(variable_ty, vec![invalid.clone()]).unwrap(),
    )
    .unwrap();
    let gradient_ty = cases
        .iter()
        .find(|c| c.tag() == "gradient")
        .unwrap()
        .payload_type()
        .clone();
    let confidence = SpeechConfidence::new(conduit_core::IeeeF32::from_value(0.5))
        .unwrap()
        .into_structured()
        .unwrap();
    let gradient = StructuredInfoValue::variant(
        ty.clone(),
        "gradient",
        StructuredInfoValue::record(
            gradient_ty,
            vec![
                StructuredFieldValue::new("confidence", confidence).unwrap(),
                StructuredFieldValue::new("value", invalid).unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    for candidate in [&variable, &gradient] {
        assert!(SpeechProbabilitySpecification::from_structured(candidate.clone()).is_err());
    }
    vec![variable, gradient]
}
pub fn replace_record(
    value: StructuredInfoValue,
    name: &str,
    replacement: StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        unreachable!()
    };
    let fields = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(
                field.name(),
                if field.name() == name {
                    replacement.clone()
                } else {
                    field.value().clone()
                },
            )
            .unwrap()
        })
        .collect();
    StructuredInfoValue::record(value.value_type().clone(), fields).unwrap()
}
