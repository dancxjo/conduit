use conduit_audio::{
    AudioAmplitudePowerRelationship, AudioAmplitudePowerRequest, AudioRelativeAmplitude,
    PreparedAmplitudePower,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn request(n: u64, d: u64) -> AudioAmplitudePowerRequest {
    AudioAmplitudePowerRequest::new(
        AudioRelativeAmplitude::new(d, n).unwrap(),
        AudioAmplitudePowerRelationship::SamePositiveProportionality,
    )
    .unwrap()
}

#[test]
fn exact_square_matches_independent_u128_reference() {
    let prepared = PreparedAmplitudePower::new().unwrap();
    for (n, d) in [
        (0, 1),
        (1, 2),
        (2, 1),
        (2, 4),
        (u32::MAX as u64, 1),
        (1, u32::MAX as u64),
        (u32::MAX as u64, u32::MAX as u64),
    ] {
        let original = request(n, d);
        let bytes = original.encode().unwrap();
        let conversion = prepared.convert(&bytes).unwrap();
        assert_eq!(
            conduit_audio::AudioAmplitudePowerEligible::decode(conversion.eligible_canonical())
                .unwrap()
                .request(),
            &original
        );
        let executed = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            conversion.source_program_hex(),
        )
        .unwrap();
        assert_eq!(
            executed.evaluate(conversion.eligible_canonical()).unwrap(),
            conversion.raw_result_canonical()
        );
        assert_eq!(
            conversion.result().encode().unwrap(),
            conversion.admitted_result_canonical()
        );
        assert!((d as u128) * (d as u128) > 0);
        assert!((d as u128) * (d as u128) <= u64::MAX as u128);
        assert_eq!(conversion.original(), &original);
        assert_eq!(conversion.original_canonical(), bytes);
        assert_eq!(
            *conversion.result().numerator() as u128,
            (n as u128) * (n as u128)
        );
        assert_eq!(
            *conversion.result().denominator() as u128,
            (d as u128) * (d as u128)
        );
    }
}

#[test]
fn full_quantity_domain_does_not_imply_square_eligibility() {
    let prepared = PreparedAmplitudePower::new().unwrap();
    for (n, d) in [
        (u32::MAX as u64 + 1, 1),
        (1, u32::MAX as u64 + 1),
        (u64::MAX, u64::MAX),
        (0, u64::MAX),
    ] {
        let original = request(n, d);
        assert!(prepared.convert(&original.encode().unwrap()).is_err());
    }
    assert!(AudioRelativeAmplitude::new(0, 1).is_err());
    assert!(prepared.convert(&[]).is_err());
}

#[test]
fn actual_native_type_and_value_sizes() {
    use conduit_audio::{AudioAmplitudePowerEligible, AudioPowerRatio};
    for (name, ty, value) in [
        (
            "AudioRelativeAmplitude",
            AudioRelativeAmplitude::semantic_type().unwrap(),
            AudioRelativeAmplitude::new(2, 1).unwrap().encode().unwrap(),
        ),
        (
            "AudioPowerRatio",
            AudioPowerRatio::semantic_type().unwrap(),
            AudioPowerRatio::new(4, 1).unwrap().encode().unwrap(),
        ),
        (
            "AudioAmplitudePowerRequest",
            AudioAmplitudePowerRequest::semantic_type().unwrap(),
            request(1, 2).encode().unwrap(),
        ),
        (
            "AudioAmplitudePowerEligible",
            AudioAmplitudePowerEligible::semantic_type().unwrap(),
            AudioAmplitudePowerEligible::new(request(1, 2))
                .unwrap()
                .encode()
                .unwrap(),
        ),
    ] {
        println!(
            "{name}: static Type={}B canonical value={}B",
            ty.canonical_bytes().unwrap().len(),
            value.len()
        );
    }
}

#[test]
fn forged_typed_request_refuses_before_arithmetic() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    fn zero_denominator(value: &StructuredInfoValue) -> StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return value.clone();
        };
        let fields = fields
            .iter()
            .map(|field| {
                let child = if field.name() == "denominator" {
                    StructuredInfoValue::leaf(
                        field.value().value_type().clone(),
                        0_u64.to_le_bytes().to_vec(),
                    )
                    .unwrap()
                } else {
                    zero_denominator(field.value())
                };
                StructuredFieldValue::new(field.name(), child).unwrap()
            })
            .collect();
        StructuredInfoValue::record(value.value_type().clone(), fields).unwrap()
    }
    let forged = zero_denominator(&request(1, 2).into_structured().unwrap())
        .canonical_bytes()
        .unwrap();
    assert!(PreparedAmplitudePower::new()
        .unwrap()
        .convert(&forged)
        .is_err());
}
