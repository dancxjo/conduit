#![cfg(feature = "reported-generation")]
//! Canonical latest-generation content and reported recovery, never publication ACK.
use conduit_core::{ActivePlayId, BootId, HostId, PlanId, Quantity};
use conduit_dht11_observation::{
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition as D},
    reported_generation::{
        self as g, PreparedReportedRecordingGeneration as Encoder,
        PreparedReportedRecordingGenerationInput as Decoder, Refusal,
        ReportedGenerationChannel as Channel,
    },
    ObservationContract, PreparedObservationQuantityForms,
};
struct Producer {
    host: HostId,
    boot: BootId,
    plan: PlanId,
    play: ActivePlayId,
}
impl Producer {
    fn new() -> Self {
        Self {
            host: "reported/host".into(),
            boot: "reported/boot".into(),
            plan: "reported/plan".into(),
            play: "reported/play".into(),
        }
    }
    fn context(&self) -> RecordingContext<'_> {
        RecordingContext {
            host: &self.host,
            boot: &self.boot,
            plan: &self.plan,
            play: &self.play,
        }
    }
}
fn channels() -> [Channel<'static>; 2] {
    [
        Channel {
            source_identity: "source/t",
            source_name: "Recorded temperature",
            channel_identity: "channel/t",
            channel_name: "Temperature",
        },
        Channel {
            source_identity: "source/h",
            source_name: "Recorded humidity",
            channel_identity: "channel/h",
            channel_name: "Humidity",
        },
    ]
}
fn chunk(p: &Producer, contract: &ObservationContract, events: &[(u64, u64, D)]) -> String {
    let mut chunk = PreparedRecordingChunk::prepare(p.context(), contract).unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    for &(sequence, time, status) in events {
        chunk
            .append(
                sequence,
                time,
                status,
                (status == D::Available).then_some(observation),
            )
            .unwrap();
    }
    chunk.seal().unwrap().into()
}
fn encode(p: &Producer, contract: &ObservationContract, json: &str) -> Vec<u8> {
    let mut encoder = Encoder::prepare(p.context(), contract, 64, channels()).unwrap();
    encoder.read_next(json).unwrap();
    encoder.encode().unwrap()
}
fn expected(contract: &ObservationContract) -> [Quantity; 2] {
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let value =
        conduit_core::validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap())
            .unwrap();
    ["temperature", "humidity"].map(|name| {
        Quantity::decode(&value.record_field(name).unwrap().unwrap().value_node()[5..]).unwrap()
    })
}
#[test]
fn original_sixty_four_readings_roundtrip_without_resealing_or_rounding() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let events = (1..=64)
        .map(|n| {
            (
                n,
                n * n * 7,
                if n % 2 == 0 {
                    D::Available
                } else {
                    D::Checksum
                },
            )
        })
        .collect::<Vec<_>>();
    let json = chunk(&p, &contract, &events);
    let canonical = encode(&p, &contract, &json);
    assert_ne!(canonical, json.as_bytes());
    assert!(canonical.len() <= g::maximum_canonical_bytes().unwrap() as usize);
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&canonical, &contract)
        .unwrap();
    assert_eq!(decoded.original_json(), json);
    assert_eq!(
        decoded.producer(),
        [
            "reported/host",
            "reported/boot",
            "reported/plan",
            "reported/play"
        ]
    );
    assert_eq!(decoded.channel(0), Some(channels()[0]));
    assert_eq!(decoded.channel(1), Some(channels()[1]));
    assert_eq!(decoded.len(), 64);
    assert!(!decoded.is_empty());
    assert_eq!(decoded.recovery().chunks, 1);
    assert_eq!(decoded.recovery().samples, 64);
    assert_eq!(decoded.recovery().samples_outside_view, 0);
    assert!(!decoded.recovery().preceding_history_known);
    let quantities = expected(&contract);
    for (index, event) in events.iter().enumerate() {
        let sample = decoded.sample(index).unwrap();
        assert_eq!(
            (
                sample.sequence,
                sample.sampled_at_boot_ms,
                sample.disposition
            ),
            *event
        );
        assert_eq!(sample.gap_before, None);
        if let Some(bytes) = sample.observation {
            let value = conduit_core::validate_canonical_structured_value(bytes).unwrap();
            for (field, expected) in ["temperature", "humidity"].into_iter().zip(quantities) {
                assert_eq!(
                    Quantity::decode(
                        &value.record_field(field).unwrap().unwrap().value_node()[5..]
                    )
                    .unwrap(),
                    expected
                );
            }
        } else {
            assert_eq!(sample.disposition, D::Checksum);
        }
    }
}
#[test]
fn recovery_keeps_only_latest_original_and_preserves_first_boundary_and_refusals() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let mut encoder = Encoder::prepare(p.context(), &contract, 3, channels()).unwrap();
    assert_eq!(encoder.encode().err(), Some(Refusal::Empty));
    encoder
        .read_next(&chunk(
            &p,
            &contract,
            &[(1, 3, D::Available), (2, 9, D::Checksum)],
        ))
        .unwrap();
    let latest = chunk(
        &p,
        &contract,
        &[(5, 11, D::ProviderFailed), (6, 100, D::Available)],
    );
    encoder.read_next(&latest).unwrap();
    let bytes = encoder.encode().unwrap();
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&bytes, &contract)
        .unwrap();
    assert_eq!(decoded.original_json(), latest);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded.recovery().samples, 4);
    assert_eq!(decoded.recovery().samples_outside_view, 2);
    assert_eq!(decoded.recovery().first_sequence, 1);
    assert_eq!(decoded.recovery().last_sequence, 6);
    assert!(!decoded.recovery().preceding_history_known);
    assert_eq!(decoded.sample(0).unwrap().gap_before, Some((3, 4)));
    let decoder = Decoder::prepare().unwrap();
    for (field, payload) in [
        ("predecessor-sequence", 5u64.to_le_bytes().to_vec()),
        ("predecessor-boot-ms", 12u64.to_le_bytes().to_vec()),
        ("recovered-first-sequence", 3u64.to_le_bytes().to_vec()),
        ("recovered-last-sequence", 7u64.to_le_bytes().to_vec()),
    ] {
        assert!(decoder
            .decode(&altered(&bytes, field, &payload), &contract)
            .is_err());
    }

    for refused in [
        latest.clone(),
        "{}".into(),
        chunk(&p, &contract, &[(7, 99, D::Available)]),
    ] {
        assert!(encoder.read_next(&refused).is_err());
        assert_eq!(encoder.encode().unwrap(), bytes);
    }
    assert_eq!(encoder.original_json(), latest);
}
#[test]
fn sixty_four_generations_report_4096_recovered_but_retain_only_latest_chunk() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let mut encoder = Encoder::prepare(p.context(), &contract, 64, channels()).unwrap();
    let mut latest = String::new();
    for generation in 0..64 {
        let events = (1..=64)
            .map(|n| {
                let sequence = generation * 64 + n;
                (sequence, sequence * 13, D::Checksum)
            })
            .collect::<Vec<_>>();
        latest = chunk(&p, &contract, &events);
        encoder.read_next(&latest).unwrap();
    }
    let before = encoder.encode().unwrap();
    assert_eq!(encoder.recovery().unwrap().samples, 4096);
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&before, &contract)
        .unwrap();
    assert_eq!(decoded.recovery().chunks, 64);
    assert_eq!(decoded.recovery().samples_outside_view, 4032);
    assert_eq!(decoded.original_json(), latest);
    assert_eq!(decoded.len(), 64);
    assert_eq!(decoded.sample(0).unwrap().sequence, 4033);
    assert_eq!(decoded.sample(0).unwrap().gap_before, None);
    assert!(encoder
        .read_next("not another admitted generation")
        .is_err());
    assert_eq!(encoder.encode().unwrap(), before);
}
fn altered(bytes: &[u8], field: &str, payload: &[u8]) -> Vec<u8> {
    use conduit_core::{
        OwnedStructuredInfoValueShape as S, StructuredFieldValue as F, StructuredInfoValue as V,
    };
    let (ty, shape) = V::from_canonical_bytes(bytes).unwrap().into_parts();
    let S::Record(fields) = shape else {
        panic!("record")
    };
    let fields = fields
        .into_iter()
        .map(|f| {
            let (name, value) = f.into_parts();
            let value = if name == field {
                V::leaf(value.value_type().clone(), payload.to_vec()).unwrap()
            } else {
                value
            };
            F::new(name, value).unwrap()
        })
        .collect();
    V::record(ty, fields).unwrap().canonical_bytes().unwrap()
}
#[test]
fn malformed_profile_context_metadata_counts_boundary_and_original_refuse() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let json = chunk(&p, &contract, &[(3, 7, D::Available)]);
    let bytes = encode(&p, &contract, &json);
    let decoder = Decoder::prepare().unwrap();
    for (field, payload) in [
        ("producer-boot", b"foreign".to_vec()),
        ("humidity-channel", b"source/t".to_vec()),
        ("producer-host", vec![b'h'; 129]),
        ("original-recording", b"{}".to_vec()),
        ("recovered-chunks", 65u16.to_le_bytes().to_vec()),
        ("recovered-samples", 4097u32.to_le_bytes().to_vec()),
        ("samples-outside-view", 1u32.to_le_bytes().to_vec()),
        ("preceding-history-known", vec![1]),
        ("predecessor-known", vec![1]),
        ("predecessor-sequence", 1u64.to_le_bytes().to_vec()),
    ] {
        assert!(
            decoder
                .decode(&altered(&bytes, field, &payload), &contract)
                .is_err(),
            "{field}"
        );
    }
    let mut foreign_profile = bytes.clone();
    let position = foreign_profile
        .windows(g::INFO_ID.len())
        .position(|window| window == g::INFO_ID.as_bytes())
        .unwrap();
    foreign_profile[position] = b'x';
    assert!(decoder.decode(&foreign_profile, &contract).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decoder.decode(&trailing, &contract).is_err());
    assert!(decoder.decode(json.as_bytes(), &contract).is_err());
    assert!(decoder
        .decode(
            &vec![0; g::maximum_canonical_bytes().unwrap() as usize + 1],
            &contract
        )
        .is_err());
    assert!(Encoder::prepare(p.context(), &contract, 65, channels()).is_err());
    let bad = Channel {
        channel_identity: "source/t",
        ..channels()[1]
    };
    assert!(Encoder::prepare(p.context(), &contract, 1, [channels()[0], bad]).is_err());
}
#[test]
fn requested_backing_models_cover_reachable_prepared_and_decoded_material() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let mut encoder = Encoder::prepare(p.context(), &contract, 1, channels()).unwrap();
    encoder
        .read_next(&chunk(&p, &contract, &[(1, 0, D::Available)]))
        .unwrap();
    let bytes = encoder.encode().unwrap();
    let decoder = Decoder::prepare().unwrap();
    let decoded = decoder.decode(&bytes, &contract).unwrap();
    assert!(
        encoder.retained_backing_bytes().unwrap() + g::encoding_workspace_bound().unwrap()
            <= Encoder::preparation_backing_bound(&contract).unwrap()
    );
    assert!(
        decoder.retained_backing_bytes().unwrap() + decoded.retained_backing_bytes().unwrap()
            <= Decoder::decoding_backing_bound(&contract).unwrap()
    );
    let ty = g::value_type().unwrap();
    let conduit_core::StructuredInfoTypeShape::Record { schema, .. } = ty.shape() else {
        panic!("record")
    };
    assert_eq!(schema.as_str(), g::INFO_ID);
    // Diagnostic only: the contract derives its bound without consulting a worker.
    eprintln!("reported-generation canonical maximum={} encodingWorkspace={} capturePreparation={} decodePreparation={}",g::maximum_canonical_bytes().unwrap(),g::encoding_workspace_bound().unwrap(),Encoder::preparation_backing_bound(&contract).unwrap(),Decoder::decoding_backing_bound(&contract).unwrap());
}

#[test]
fn every_original_disposition_survives_the_new_canonical_envelope() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let dispositions = [
        D::Available,
        D::CaptureTimeout,
        D::ProviderFailed,
        D::GeometryRefused,
        D::ShortFrame,
        D::MalformedLength,
        D::Checksum,
        D::InvalidPayload,
    ];
    let events = dispositions
        .iter()
        .enumerate()
        .map(|(index, disposition)| {
            let sequence = index as u64 + 1;
            (sequence, sequence * sequence, *disposition)
        })
        .collect::<Vec<_>>();
    let original = chunk(&p, &contract, &events);
    let bytes = encode(&p, &contract, &original);
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&bytes, &contract)
        .unwrap();
    assert_eq!(decoded.original_json(), original);
    for (index, expected) in dispositions.into_iter().enumerate() {
        let sample = decoded.sample(index).unwrap();
        assert_eq!(sample.disposition, expected);
        assert_eq!(sample.observation.is_some(), expected == D::Available);
    }
}
