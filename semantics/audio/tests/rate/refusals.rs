use super::*;
#[test]
fn full_source_domains_do_not_imply_projection_eligibility() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    for (n, d, rate) in [
        (u64::MAX, 1, 1),
        (1, u64::MAX, 1),
        (u32::MAX as u64 + 1, u32::MAX as u64 + 1, 8000),
        (1, 1, 192001),
        (1, 1, u64::MAX),
    ] {
        let request = request(n, d, rate);
        assert!(prepared.project(&request.encode().unwrap()).is_err());
    }
    assert!(AudioTimeFraction::new(0, 1).is_err());
    assert!(AudioSampleProjectionQuantity::cycle(1, 0).is_err());
    assert!(AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 0).is_err());
}
#[test]
fn foreign_rate_anchor_and_unreduced_denominator_refuse_before_append() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    let cursor = AudioCumulativeFrameCursor::new(
        AudioCumulativeFrameBasis::new(basis(8000), 3).unwrap(),
        2,
        2666,
    )
    .unwrap();
    for request in [
        request(1, 3, 16000),
        request(2, 6, 8000),
        AudioSampleProjectionRequest::new(
            AudioSampleRateBasis::new(
                AudioTrajectoryAnchor::new(
                    AudioOriginIdentity::new(2).unwrap(),
                    AudioTimelineIdentity::new(1).unwrap(),
                )
                .unwrap(),
                AudioFrameQuantization::Floor,
                8000,
            )
            .unwrap(),
            duration(1, 3),
        )
        .unwrap(),
    ] {
        let input = AudioCumulativeFrameRequest::new(request, cursor.clone()).unwrap();
        assert!(matches!(
            prepared.append(&input.encode().unwrap()),
            Err(AudioRateProjectionRefusal::ForeignBasis)
        ));
    }
    assert!(AudioCumulativeFrameCursor::new(cursor.basis().clone(), 3, 0).is_err());
}
#[test]
fn cumulative_profile_checks_carry_and_cursor_before_multiply_add() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    let denominator = u32::MAX as u64;
    let prior = 9223372036854775807_u64;
    let cursor = AudioCumulativeFrameCursor::new(
        AudioCumulativeFrameBasis::new(basis(192000), denominator).unwrap(),
        denominator - 1,
        prior,
    )
    .unwrap();
    let append = request(denominator, denominator, 192000);
    let receipt = prepared
        .append(
            &AudioCumulativeFrameRequest::new(append, cursor)
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
    let total = denominator as u128 * 192000 + denominator as u128 - 1;
    assert_eq!(
        *receipt.result().raw().whole_frames() as u128,
        prior as u128 + total / denominator as u128
    );
    assert_eq!(
        *receipt.result().raw().remainder_numerator() as u128,
        total % denominator as u128
    );
    for prior in [9223372036854775808_u64, u64::MAX] {
        let cursor = AudioCumulativeFrameCursor::new(
            AudioCumulativeFrameBasis::new(basis(8000), 1).unwrap(),
            0,
            prior,
        )
        .unwrap();
        let input = AudioCumulativeFrameRequest::new(request(1, 1, 8000), cursor).unwrap();
        assert!(prepared.append(&input.encode().unwrap()).is_err());
    }
}
#[test]
fn finite_chain_and_native_result_laws_refuse_forged_values() {
    assert!(
        BoundedSequence::<AudioSampleProjectionQuantity, 16>::try_from_iter(vec![
            duration(1, 3);
            17
        ])
        .is_err()
    );
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    assert!(prepared.project(&[]).is_err());
    assert!(prepared.append(&[]).is_err());
    let receipt = prepared
        .project(&request(1, 3, 8000).encode().unwrap())
        .unwrap();
    let wrong = AudioSampleProjectionResult::new(
        AudioFrameGridFidelity::Exact,
        receipt.result().fraction().clone(),
        receipt.result().raw().clone(),
        receipt.original().clone(),
    );
    assert!(wrong.is_err());
    // A foreign declared policy is a foreign Native variant, never a fallback.
    use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
    let policy = AudioFrameQuantization::Floor.into_structured().unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = policy.shape() else {
        panic!("policy")
    };
    assert!(
        StructuredInfoValue::variant(policy.value_type().clone(), "nearest", payload.clone())
            .is_err()
    );
}

#[test]
fn forged_full_cursor_frame_refuses_before_source_arithmetic() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    fn corrupt_carry(value: &StructuredInfoValue) -> StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return value.clone();
        };
        let fields = fields
            .iter()
            .map(|field| {
                let value = if field.name() == "remainder_numerator" {
                    StructuredInfoValue::leaf(
                        field.value().value_type().clone(),
                        3_u64.to_le_bytes().to_vec(),
                    )
                    .unwrap()
                } else {
                    corrupt_carry(field.value())
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect();
        StructuredInfoValue::record(value.value_type().clone(), fields).unwrap()
    }
    let cursor = AudioCumulativeFrameCursor::new(
        AudioCumulativeFrameBasis::new(basis(8000), 3).unwrap(),
        2,
        2666,
    )
    .unwrap();
    let request = AudioCumulativeFrameRequest::new(request(1, 3, 8000), cursor)
        .unwrap()
        .into_structured()
        .unwrap();
    let forged = corrupt_carry(&request).canonical_bytes().unwrap();
    assert!(PreparedAudioSampleProjection::new()
        .unwrap()
        .append(&forged)
        .is_err());
}
