#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{feature_bundle::*, semantic::*};
fn bundle(entries: &[(&str, FeatureSpecification)]) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter(entries.iter().map(|(identity, specification)| {
            SpeechFeature::new(
                SpeechFeatureId::new((*identity).into()).unwrap(),
                specification.clone(),
            )
            .unwrap()
        }))
        .unwrap(),
    )
    .unwrap()
}
fn known(value: bool) -> FeatureSpecification {
    FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap()
}
#[test]
fn exact_keys_and_missing_values_keep_original_receipts_without_defaults() {
    let requirements = bundle(&[
        ("voice", known(true)),
        ("round", FeatureSpecification::unspecified()),
        ("é", known(true)),
        ("uncertain", FeatureSpecification::unknown()),
    ]);
    let observations = bundle(&[
        ("e\u{301}", known(true)),
        ("extra", known(false)),
        ("voice", known(false)),
    ]);
    let compared = compare_feature_bundle(&requirements, &observations).unwrap();
    assert!(core::ptr::eq(compared.requirements(), &requirements));
    assert!(core::ptr::eq(compared.observations(), &observations));
    let receipts: Vec<_> = compared.comparisons().collect();
    for (index, receipt) in receipts.iter().enumerate() {
        assert!(core::ptr::eq(
            receipt.requirement(),
            &requirements.get().as_slice()[index]
        ));
    }
    assert_eq!(receipts[0].decision(), &SpeechContextDecision::Mismatched);
    assert!(core::ptr::eq(
        receipts[0].observation().unwrap(),
        &observations.get().as_slice()[2]
    ));
    assert!(receipts[0].checked_key().is_some());
    assert_eq!(receipts[1].decision(), &SpeechContextDecision::Matched);
    assert_eq!(
        receipts[2].decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(
        receipts[3].decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    for receipt in &receipts[1..] {
        assert!(receipt.observation().is_none());
        assert!(receipt.checked_key().is_none());
    }
}
#[test]
fn duplicate_keys_refuse_on_the_exact_side_before_returning_receipts() {
    let empty = bundle(&[]);
    let duplicate = bundle(&[
        ("voice", known(true)),
        ("voice", FeatureSpecification::unspecified()),
    ]);
    assert!(matches!(
        compare_feature_bundle(&duplicate, &empty),
        Err(FeatureBundleRefusal::Bundle {
            side: FeatureBundleSide::Requirement,
            ..
        })
    ));
    assert!(matches!(
        compare_feature_bundle(&empty, &duplicate),
        Err(FeatureBundleRefusal::Bundle {
            side: FeatureBundleSide::Observation,
            ..
        })
    ));
    assert!(SpeechFeatureIdentityMatch::new(
        SpeechFeatureId::new("a".into()).unwrap(),
        SpeechFeatureId::new("b".into()).unwrap()
    )
    .is_err());
}
#[test]
fn empty_and_sixteen_feature_bundles_have_exact_fixed_receipt_counts() {
    let empty = bundle(&[]);
    assert_eq!(
        compare_feature_bundle(&empty, &empty)
            .unwrap()
            .comparisons()
            .count(),
        0
    );
    let keys: Vec<_> = (0..16).map(|index| format!("feature/{index}")).collect();
    let requirements = bundle(
        &keys
            .iter()
            .map(|key| (key.as_str(), known(true)))
            .collect::<Vec<_>>(),
    );
    let observations = bundle(
        &keys
            .iter()
            .rev()
            .map(|key| (key.as_str(), known(true)))
            .collect::<Vec<_>>(),
    );
    let compared = compare_feature_bundle(&requirements, &observations).unwrap();
    assert_eq!(compared.comparisons().count(), 16);
    for (index, receipt) in compared.comparisons().enumerate() {
        assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
        assert!(core::ptr::eq(
            receipt.observation().unwrap(),
            &observations.get().as_slice()[15 - index]
        ));
    }
}
