//! Domain Info decoding and allocating semantic preparation, not installed ACK proof.
use super::*;
use conduit_dht11_observation::{
    recording::{PreparedRecordingChunk, RecordingContext},
    reported_generation::{PreparedReportedRecordingGeneration, ReportedGenerationChannel},
    PreparedObservationQuantityForms,
};
use conduit_face::{Face, FaceBasis, PreparedQuantitySeries};
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/inspector".into(),
        plan_id: "plan/inspector".into(),
        active_play_id: "play/inspector".into(),
        required_interaction_context: None,
    }
}
fn fixture() -> (Vec<u8>, String, [Quantity; 2]) {
    let (host, boot, plan, play) = (
        "reported/host".into(),
        "reported/boot".into(),
        "reported/plan".into(),
        "reported/play".into(),
    );
    let context = || RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    let value = validate_canonical_structured_value(observation).unwrap();
    let expected = ["temperature", "humidity"].map(|name| {
        Quantity::decode(&value.record_field(name).unwrap().unwrap().value_node()[5..]).unwrap()
    });
    let mut earlier = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    earlier
        .append(1, 0, D::Available, Some(observation))
        .unwrap();
    let mut latest = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    for index in 0..64_u64 {
        let disposition = status(index as usize);
        latest
            .append(
                index + 4,
                index * index * 7 + 1,
                disposition,
                (disposition == D::Available).then_some(observation),
            )
            .unwrap();
    }
    let original = latest.seal().unwrap().to_owned();
    let mut encoder = PreparedReportedRecordingGeneration::prepare(
        context(),
        &contract,
        64,
        [
            ReportedGenerationChannel {
                source_identity: "source/t",
                source_name: "Temperature source",
                channel_identity: "series/t",
                channel_name: "Temperature",
            },
            ReportedGenerationChannel {
                source_identity: "source/h",
                source_name: "Humidity source",
                channel_identity: "series/h",
                channel_name: "Humidity",
            },
        ],
    )
    .unwrap();
    encoder.read_next(earlier.seal().unwrap()).unwrap();
    encoder.read_next(&original).unwrap();
    (encoder.encode().unwrap(), original, expected)
}
fn status(index: usize) -> D {
    [
        D::Available,
        D::CaptureTimeout,
        D::ProviderFailed,
        D::GeometryRefused,
        D::ShortFrame,
        D::MalformedLength,
        D::Checksum,
        D::InvalidPayload,
    ][index % 8]
}
fn face(fragment: FaceFragment) -> Face {
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/inspector".into()),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some("expanded/inspector".into()),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap()
}
#[test]
fn exact_domain_generation_preserves_all_sixty_four_readings_outcomes_and_recovery() {
    let (encoded, _, expected) = fixture();
    let projector = ReportedGenerationProjector::prepare().unwrap();
    let fragment = projector
        .project(basis(), &encoded, "Recorded history")
        .unwrap();
    let backing = projector.fragment_backing_bytes(&fragment).unwrap();
    assert!(backing <= projector.fragment_backing_bound());
    assert!(projector.retained_backing_bytes().unwrap() > 0);
    assert!(projector.projection_workspace_bytes().unwrap() > backing);
    assert!(
        ReportedGenerationProjector::workspace_bytes().unwrap()
            >= projector.retained_backing_bytes().unwrap()
                + projector.projection_workspace_bytes().unwrap()
    );
    let face = face(fragment);
    for ((source, channel), quantity) in [("source/t", "series/t"), ("source/h", "series/h")]
        .into_iter()
        .zip(expected)
    {
        let series = PreparedQuantitySeries::prepare(&face, channel, 64).unwrap();
        assert_eq!(
            series.producer(),
            [
                "reported/host",
                "reported/boot",
                "reported/plan",
                "reported/play"
            ]
        );
        assert_eq!(series.samples().len(), 64);
        for (index, sample) in series.samples().iter().enumerate() {
            assert_eq!(sample.sequence, index as u64 + 4);
            assert_eq!(sample.boot_ms, (index as u64).pow(2) * 7 + 1);
            assert_eq!(sample.gap_before, (index == 0).then_some((2, 3)));
            assert_eq!(
                sample.value,
                (status(index) == D::Available).then_some(quantity)
            );
            assert_eq!(sample.unavailable_reason, reason(status(index)));
        }
        for (name, value) in [
            ("recovered-chunks", V::Count(2)),
            ("recovered-samples", V::Count(65)),
            ("visible-samples", V::Count(64)),
            ("samples-outside-view", V::Count(1)),
            ("recovered-first-sequence", V::Count(1)),
            ("recovered-last-sequence", V::Count(67)),
            ("preceding-history-known", V::Flag(false)),
        ] {
            assert!(face
                .properties
                .iter()
                .any(|p| p.subject == source && p.name == name && p.value == value));
        }
        let plot = series.project(None, 16).unwrap();
        assert_eq!(plot.source_samples, 64);
        assert_eq!(plot.points.len(), 8);
        assert_eq!(plot.unavailable_samples, 56);
        assert_eq!(plot.omitted_samples, "1");
        assert!(plot.publication_reference.is_none() && plot.publication_evidence.is_none());
    }
    assert_eq!(
        face.basis.active_play_id.unwrap().as_str(),
        "play/inspector"
    );
}
#[test]
fn raw_json_wrong_domain_encoding_and_invalid_inspecting_basis_refuse() {
    let (encoded, json, _) = fixture();
    let projector = ReportedGenerationProjector::prepare().unwrap();
    for invalid in [json.as_bytes(), b"not a domain Info", &[]] {
        assert!(projector.project(basis(), invalid, "History").is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(projector.project(basis(), &trailing, "History").is_err());
    let mut invalid = basis();
    invalid.required_interaction_context = Some("foreign/context".into());
    assert!(projector.project(invalid, &encoded, "History").is_err());
    assert!(projector.project(basis(), &encoded, "").is_err());
    assert!(projector.project(basis(), &encoded, "History").is_ok());
}
