#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    default_output_features::*,
    output_features::{OutputFeatureLayer, OutputFeatureRefusal},
    semantic::*,
};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod native;
fn feature(key: &str, value: FeatureSpecification) -> SpeechFeature {
    SpeechFeature::new(SpeechFeatureId::new(key.into()).unwrap(), value).unwrap()
}
fn known(value: bool) -> FeatureSpecification {
    FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap()
}
fn bundle(values: impl IntoIterator<Item = SpeechFeature>) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(BoundedSequence::try_from_iter(values).unwrap()).unwrap()
}
fn phone(features: SpeechFeatureBundle) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        features,
        native::id("phone/t"),
        "t".into(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
#[test]
fn occurrence_overrides_whole_definition_specification_preserving_borrowed_layers_and_order() {
    let phone = phone(bundle([
        feature("shared", known(true)),
        feature("definition-only", known(false)),
    ]));
    for value in [
        known(false),
        FeatureSpecification::unknown(),
        FeatureSpecification::unspecified(),
        FeatureSpecification::not_applicable(),
        FeatureSpecification::variable(
            BoundedSequence::try_from_iter([SpeechFeatureValue::boolean(true).unwrap()]).unwrap(),
        )
        .unwrap(),
        FeatureSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            SpeechFeatureValue::boolean(true).unwrap(),
        )
        .unwrap(),
    ] {
        let defaults = bundle([
            feature("shared", value),
            feature("occurrence-only", known(true)),
        ]);
        let prepared = prepare_default_output_features(&defaults, &phone).unwrap();
        assert!(core::ptr::eq(prepared.default_features(), &defaults));
        assert!(core::ptr::eq(prepared.phone_definition(), &phone));
        let entries: Vec<_> = prepared.features().collect();
        assert_eq!(entries.len(), 3);
        assert!(core::ptr::eq(
            entries[0].feature(),
            &defaults.get().as_slice()[0]
        ));
        assert_eq!(entries[0].layer(), OutputFeatureLayer::DefaultRealization);
        assert!(core::ptr::eq(
            entries[1].feature(),
            &phone.features().get().as_slice()[1]
        ));
        assert_eq!(entries[1].layer(), OutputFeatureLayer::PhoneDefinition);
        assert!(core::ptr::eq(
            entries[2].feature(),
            &defaults.get().as_slice()[1]
        ));
    }
}
#[test]
fn duplicate_keys_and_merged_capacity_refuse_without_normalizing_unicode_ids() {
    let empty = bundle([]);
    let duplicate = bundle([feature("same", known(true)), feature("same", known(false))]);
    let plain = phone(bundle([]));
    assert!(matches!(
        prepare_default_output_features(&duplicate, &plain),
        Err(OutputFeatureRefusal::Bundle {
            layer: OutputFeatureLayer::DefaultRealization,
            ..
        })
    ));
    let rich = phone(duplicate);
    assert!(matches!(
        prepare_default_output_features(&empty, &rich),
        Err(OutputFeatureRefusal::Bundle {
            layer: OutputFeatureLayer::PhoneDefinition,
            ..
        })
    ));
    let rich = phone(bundle(
        (0..16).map(|n| feature(&format!("feature/{n}"), known(true))),
    ));
    let same = bundle([feature("feature/0", FeatureSpecification::unknown())]);
    assert_eq!(
        prepare_default_output_features(&same, &rich)
            .unwrap()
            .features()
            .count(),
        16
    );
    let seventeenth = bundle([feature("feature/16", known(true))]);
    assert!(matches!(
        prepare_default_output_features(&seventeenth, &rich),
        Err(OutputFeatureRefusal::Capacity)
    ));
    let rich = phone(bundle([feature("é", known(true))]));
    let separate = bundle([feature("e\u{301}", known(false))]);
    assert_eq!(
        prepare_default_output_features(&separate, &rich)
            .unwrap()
            .features()
            .count(),
        2
    );
}
