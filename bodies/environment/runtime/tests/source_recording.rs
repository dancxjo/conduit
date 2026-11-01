#![cfg(feature = "source-recording")]
use conduit_core as c;
use conduit_environment_measurement::{source_observation::*, EnvironmentalReading};
use conduit_environment_runtime::source_recording::*;
fn context() -> SourceRecordingContext {
    let host: c::HostId = "host/actual-owner".into();
    let boot: c::BootId = "boot/actual-owner".into();
    let plan: c::PlanId = "plan/source".into();
    SourceRecordingContext {
        body_id: "body/hamm".into(),
        attachment_id: "green".into(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        plan_id: plan.clone(),
        capability_id: "source/green".into(),
        provider_instance_id: "provider/green".into(),
        provider_generation: 1,
        fragment_id: "fragment/source".into(),
        placement_id: "placement/source".into(),
        configuration_id: "sha256:configuration".into(),
        firmware_sha256: [42; 32],
        native_play: c::bind_active_play(&plan, &host, &boot, 1),
    }
}
fn sample(counter: u64, time: u64) -> Vec<u8> {
    let contract = ObservationContract::prepare().unwrap();
    PreparedObservationCodec::prepare(&contract)
        .unwrap()
        .encode(SourceObservation {
            observed_at_host_boot_ms: time,
            outcome: SourceOutcome::Available(SourceSample {
                reading: EnvironmentalReading::centi_celsius_centi_pascal(
                    2422,
                    10_134_081,
                    Some(46535),
                )
                .unwrap(),
                source_counter: counter,
                reported_source_ms: Some(825388),
            }),
        })
        .unwrap()
        .to_vec()
}
#[test]
fn preserves_full_canonical_source_pressure_and_exact_native_context() {
    let scope = context();
    let input = sample(28, 9000);
    assert!(input.len() > FORM_BYTES);
    let mut snapshot = SourceRecordingSnapshot::prepare(scope.clone()).unwrap();
    assert_eq!(snapshot.content(), Err(Refusal::Empty));
    assert_eq!(snapshot.append(1, &input).unwrap(), Append::Added);
    assert_eq!(snapshot.append(1, &input).unwrap(), Append::Duplicate);
    let restored = SourceRecordingSnapshot::read(snapshot.content().unwrap(), &scope).unwrap();
    assert_eq!(restored.context(), &scope);
    assert_eq!(restored.observation(0).unwrap(), (1, input));
    let observation = ObservationContract::prepare()
        .unwrap()
        .decode(&restored.observation(0).unwrap().1)
        .unwrap();
    let SourceOutcome::Available(value) = observation.outcome else {
        panic!()
    };
    assert_eq!(
        value.reading.atmospheric_pressure(),
        conduit_environment_measurement::ReadingField::Available(
            c::Quantity::from_decimal(10_134_081, -2, c::Unit::Pascal).unwrap()
        )
    );
}
#[test]
fn wrong_body_session_play_and_noncanonical_content_refuse() {
    let scope = context();
    let mut snapshot = SourceRecordingSnapshot::prepare(scope.clone()).unwrap();
    snapshot.append(1, &sample(28, 9000)).unwrap();
    for field in 0..4 {
        let mut changed = scope.clone();
        match field {
            0 => changed.body_id = "body/foreign".into(),
            1 => changed.provider_generation += 1,
            2 => changed.firmware_sha256[0] ^= 1,
            _ => changed.native_play.play_sequence += 1,
        };
        assert!(SourceRecordingSnapshot::read(snapshot.content().unwrap(), &changed).is_err());
    }
    let mut malformed = snapshot.content().unwrap().to_vec();
    malformed.push(b' ');
    assert!(matches!(
        SourceRecordingSnapshot::read(&malformed, &scope),
        Err(Refusal::Encoding)
    ));
    let mut invalid = scope;
    invalid.native_play.boot_id = "boot/foreign".into();
    assert!(matches!(
        SourceRecordingSnapshot::prepare(invalid),
        Err(Refusal::Scope)
    ));
}
#[test]
fn failed_appends_preserve_previous_generation_and_finite_bound() {
    let mut snapshot = SourceRecordingSnapshot::prepare(context()).unwrap();
    snapshot.append(1, &sample(28, 9000)).unwrap();
    let original = snapshot.content().unwrap().to_vec();
    assert_eq!(
        snapshot.append(1, &sample(29, 9001)),
        Err(Refusal::Conflict)
    );
    assert_eq!(snapshot.append(2, &sample(27, 9001)), Err(Refusal::Order));
    assert_eq!(snapshot.append(2, &sample(29, 8999)), Err(Refusal::Order));
    assert_eq!(snapshot.append(2, &[0; 129]), Err(Refusal::Observation));
    assert_eq!(snapshot.content().unwrap(), original);
    let mut count = 1;
    loop {
        let previous = snapshot.content().unwrap().to_vec();
        match snapshot.append(count + 1, &sample(count + 28, 9000 + count)) {
            Ok(Append::Added) => count += 1,
            Err(Refusal::Capacity) => {
                assert_eq!(snapshot.content().unwrap(), previous);
                break;
            }
            other => panic!("{other:?}"),
        }
    }
    assert!(count <= MAXIMUM_SAMPLES as u64);
    assert!(snapshot.content().unwrap().len() <= 4096);
}
#[test]
fn cumulative_context_budget_refuses_without_enlargement() {
    let mut scope = context();
    let long = "x".repeat(256);
    scope.body_id = long.clone();
    scope.attachment_id = long.clone();
    scope.capability_id = long.clone().into();
    scope.provider_instance_id = long.clone().into();
    scope.configuration_id = long.clone();
    scope.fragment_id = long.clone().into();
    scope.placement_id = long.clone().into();
    // Context identifiers are finite individually; the whole content is still
    // admitted by its original4096-byte ceiling, including canonical samples.
    if let Ok(mut snapshot) = SourceRecordingSnapshot::prepare(scope) {
        for sequence in 1..=17 {
            if snapshot
                .append(sequence, &sample(sequence, sequence))
                .is_err()
            {
                break;
            }
        }
        assert!(snapshot.content().unwrap().len() <= 4096);
    }
}

#[test]
fn candidate_decode_preserves_evidence_without_replacing_pinned_context_checks() {
    let scope = context();
    let mut snapshot = SourceRecordingSnapshot::prepare(scope.clone()).unwrap();
    snapshot.append(1, &sample(28, 9000)).unwrap();
    let bytes = snapshot.content().unwrap();
    let candidate = SourceRecordingSnapshot::decode_candidate(bytes).unwrap();
    assert_eq!(candidate.context(), &scope);
    assert_eq!(candidate.content().unwrap(), bytes);
    let mut foreign = scope.clone();
    foreign.body_id = "body/other".into();
    assert!(matches!(
        SourceRecordingSnapshot::read(bytes, &foreign),
        Err(Refusal::Scope)
    ));
    let mut malformed = bytes.to_vec();
    malformed.push(b' ');
    assert!(matches!(
        SourceRecordingSnapshot::decode_candidate(&malformed),
        Err(Refusal::Encoding)
    ));
    let mut malformed: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    malformed["context"]["native_play"]["boot_id"] = "boot/contradictory".into();
    assert!(matches!(
        SourceRecordingSnapshot::decode_candidate(&serde_json::to_vec(&malformed).unwrap()),
        Err(Refusal::Scope)
    ));
    assert!(matches!(
        SourceRecordingSnapshot::decode_candidate(&[0; MAXIMUM_BYTES + 1]),
        Err(Refusal::Capacity)
    ));
}
