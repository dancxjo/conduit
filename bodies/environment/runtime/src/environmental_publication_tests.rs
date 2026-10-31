#![cfg(feature = "face-projection")]
use crate::{
    environmental_publication::EnvironmentalPublicationBack,
    environmental_publication_face::{Refusal, RetainedEnvironmentalFace},
};
use alloc::vec;
use conduit_core::{Quantity, Unit};
use conduit_environment_measurement::{source_observation::*, EnvironmentalReading};
use conduit_face::{FaceBasis, FacePropertyValue};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
fn stage(back: &mut EnvironmentalPublicationBack, bytes: &[u8]) -> StepOutcome {
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        })],
        [false],
        [None],
        None,
        8,
    );
    let result = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    if result == StepOutcome::Progress {
        assert!(io.test_consumed(PortId(0)));
    }
    result
}
fn commit(back: &mut EnvironmentalPublicationBack) {
    <EnvironmentalPublicationBack as StepBack<1>>::step_committed(back)
}
fn basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/test".into()),
        checked_plot_id: Some("checked/test".into()),
        expanded_plot_id: Some("expanded/test".into()),
        plan_id: Some("plan/test".into()),
        active_play_id: Some("play/test".into()),
        sign_ids: vec![],
    }
}
fn bme() -> SourceObservation {
    SourceObservation {
        observed_at_host_boot_ms: 100,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::centi_celsius_centi_pascal(
                2422,
                10_134_081,
                Some(46535),
            )
            .unwrap(),
            source_counter: 28,
            reported_source_ms: Some(825388),
        }),
    }
}
#[test]
fn only_kernel_commit_exposes_exact_generic_reading_and_ordinary_pressure_face() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let bytes = codec.encode(bme()).unwrap();
    let mut back = make_back(2).unwrap();
    let mut face = RetainedEnvironmentalFace::prepare(basis(), &scope(), 1000).unwrap();
    assert!(back.snapshot().is_none());
    assert_eq!(stage(&mut back, bytes), StepOutcome::Progress);
    assert!(back.snapshot().is_none());
    commit(&mut back);
    let snapshot = back.snapshot().unwrap();
    assert_eq!(snapshot.value(), bme());
    assert_eq!(snapshot.canonical(), bytes);
    face.publish(snapshot, 120).unwrap();
    let actual = face.current();
    assert_eq!(actual.basis, basis());
    actual.validate().unwrap();
    let pressure = actual
        .properties
        .iter()
        .find(|p| p.name == "atmospheric-pressure")
        .unwrap();
    let FacePropertyValue::TypedValue { bytes, .. } = &pressure.value else {
        panic!()
    };
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::from_decimal(10_134_081, -2, Unit::Pascal).unwrap()
    );
    assert_eq!(face.publish(snapshot, 121), Err(Refusal::Sequence));
    let current = face.at(1100).unwrap();
    assert!(current
        .properties
        .iter()
        .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(false)));
}
#[test]
fn cancelled_or_invalid_input_never_publishes_and_finite_capacity_is_enforced() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let bytes = codec.encode(bme()).unwrap();
    let mut back = make_back(1).unwrap();
    assert!(matches!(stage(&mut back, b"invalid"), StepOutcome::Fail(_)));
    assert!(back.snapshot().is_none());
    assert_eq!(stage(&mut back, bytes), StepOutcome::Progress);
    <EnvironmentalPublicationBack as StepBack<1>>::cancel(&mut back);
    commit(&mut back);
    assert!(back.snapshot().is_none());
    let mut bounded = make_back(1).unwrap();
    assert_eq!(stage(&mut bounded, bytes), StepOutcome::Progress);
    commit(&mut bounded);
    assert!(matches!(stage(&mut bounded, bytes), StepOutcome::Fail(_)));
}
#[test]
fn dht_has_no_pressure_and_failures_preserve_reading_without_claiming_freshness() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = SourceObservation {
        observed_at_host_boot_ms: 100,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::temperature_humidity(
                Quantity::new(24, Unit::Celsius),
                Quantity::new(36, Unit::Percent),
            )
            .unwrap(),
            source_counter: 1,
            reported_source_ms: None,
        }),
    };
    let mut back = make_back(3).unwrap();
    let mut face = RetainedEnvironmentalFace::prepare(basis(), &scope(), 1000).unwrap();
    stage(&mut back, codec.encode(value).unwrap());
    commit(&mut back);
    face.publish(back.snapshot().unwrap(), 101).unwrap();
    assert!(!face
        .current()
        .properties
        .iter()
        .any(|p| p.name == "atmospheric-pressure"));
    let failure = SourceObservation {
        observed_at_host_boot_ms: 200,
        outcome: SourceOutcome::Unavailable(SourceFailure::ProviderFailed),
    };
    stage(&mut back, codec.encode(failure).unwrap());
    commit(&mut back);
    face.publish(back.snapshot().unwrap(), 201).unwrap();
    assert!(face
        .current()
        .properties
        .iter()
        .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(false)));
    assert!(face
        .current()
        .properties
        .iter()
        .any(|p| p.name == "temperature"));
    assert_eq!(face.at(199).unwrap_err(), Refusal::Clock);
}

fn scope() -> crate::environmental_publication::PublicationScope {
    crate::environmental_publication::PublicationScope {
        host: "host/test".into(),
        boot: "boot/test".into(),
        plan: "plan/test".into(),
        play: "play/test".into(),
        source: "source/test".into(),
        checked: "checked/test".into(),
        expanded: "expanded/test".into(),
    }
}
fn make_back(
    maximum: u32,
) -> Result<EnvironmentalPublicationBack, conduit_core::StructuredInfoRefusal> {
    EnvironmentalPublicationBack::prepare(scope(), maximum)
}

#[test]
fn a_different_producer_scope_cannot_update_the_retained_face() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let mut wrong = scope();
    wrong.boot = "boot/other".into();
    let mut back = EnvironmentalPublicationBack::prepare(wrong, 1).unwrap();
    stage(&mut back, codec.encode(bme()).unwrap());
    commit(&mut back);
    let mut face = RetainedEnvironmentalFace::prepare(basis(), &scope(), 1000).unwrap();
    let before = face.current().clone();
    assert_eq!(
        face.publish(back.snapshot().unwrap(), 101),
        Err(Refusal::Basis)
    );
    assert_eq!(face.current(), &before);
}

#[test]
fn retained_publication_keeps_forms_instead_of_rich_quantity_models() {
    assert!(core::mem::size_of::<EnvironmentalPublicationBack>() < 1024);
    assert!(core::mem::size_of::<super::CommittedEnvironmentalObservation<'_>>() < 128);
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let mut back = make_back(3).unwrap();
    let value = bme();
    stage(&mut back, codec.encode(value).unwrap());
    commit(&mut back);
    assert!(matches!(
        stage(&mut back, codec.encode(value).unwrap()),
        StepOutcome::Fail(_)
    ));
    assert_eq!(back.snapshot().unwrap().sequence(), 1);
}
