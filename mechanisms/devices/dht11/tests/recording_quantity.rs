#![cfg(feature = "quantity-history")]
//! Actual recorder content becomes a finite reported view, never an ACK/store.
use conduit_core::{
    validate_canonical_structured_value, ActivePlayId, BootId, HostId, PlanId, Quantity,
};
use conduit_data::{QuantityObservationOutcome as Outcome, ResolvedQuantityObservationSnapshot};
use conduit_dht11_observation::{
    quantity_history::{
        EnvironmentQuantityChannel as Channel, EnvironmentQuantityRefusal as Refusal,
        PreparedEnvironmentQuantityObservations as Adapter,
    },
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition as D},
    ObservationContract, PreparedObservationQuantityForms,
};
use conduit_plot::rust_binding::NativeRustBinding;
struct Producer {
    host: HostId,
    boot: BootId,
    plan: PlanId,
    play: ActivePlayId,
}
impl Producer {
    fn new() -> Self {
        Self {
            host: "reported/producer".into(),
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
            source_identity: "sensor/temperature",
            source_name: "Original temperature",
            channel_identity: "series/temperature",
            channel_name: "Temperature",
        },
        Channel {
            source_identity: "sensor/humidity",
            source_name: "Original humidity",
            channel_identity: "series/humidity",
            channel_name: "Relative humidity",
        },
    ]
}
fn adapter(p: &Producer, contract: &ObservationContract, tail: usize, chunks: u16) -> Adapter {
    let [temperature, humidity] = channels();
    Adapter::prepare(p.context(), contract, chunks, temperature, humidity, tail).unwrap()
}
fn chunk(p: &Producer, contract: &ObservationContract, events: &[(u64, u64, D)]) -> String {
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(p.context(), contract).unwrap();
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
fn expected(contract: &ObservationContract) -> [Quantity; 2] {
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let value =
        validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap()).unwrap();
    ["temperature", "humidity"].map(|name| {
        Quantity::decode(&value.record_field(name).unwrap().unwrap().value_node()[5..]).unwrap()
    })
}
fn reasons() -> [(D, Option<&'static str>); 8] {
    [
        (D::Available, None),
        (D::CaptureTimeout, Some("capture-timeout")),
        (D::ProviderFailed, Some("provider-failed")),
        (D::GeometryRefused, Some("geometry-refused")),
        (D::ShortFrame, Some("short-frame")),
        (D::MalformedLength, Some("malformed-length")),
        (D::Checksum, Some("checksum")),
        (D::InvalidPayload, Some("invalid-payload")),
    ]
}
#[test]
fn selected_tail_preserves_both_exact_quantities_all_outcomes_and_irregular_boot_time() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let events = (0..20)
        .map(|n| {
            (
                n + 1 + if n >= 15 { 2 } else { 0 },
                n * n * 7,
                reasons()[n as usize % 8].0,
            )
        })
        .collect::<Vec<_>>();
    let mut view = adapter(&p, &contract, 8, 1);
    view.read_next(&chunk(&p, &contract, &events)).unwrap();
    let snapshot = view.snapshot().unwrap();
    let quantities = expected(&contract);
    for ((batch, names), quantity) in snapshot.batches.iter().zip(channels()).zip(quantities) {
        assert_eq!(batch.source_identity, names.source_identity);
        assert_eq!(batch.source_name, names.source_name);
        assert_eq!(batch.channel_identity, names.channel_identity);
        assert_eq!(batch.channel_name, names.channel_name);
        assert_eq!(
            (
                &batch.producer_host,
                &batch.producer_boot,
                &batch.producer_plan,
                &batch.producer_play
            ),
            (&p.host.0, &p.boot.0, &p.plan.0, &p.play.0)
        );
        assert_eq!(batch.samples_outside_view, 12);
        assert_eq!(batch.observations.len(), 8);
        for (sample, (index, event)) in batch
            .observations
            .iter()
            .zip(events.iter().enumerate().skip(12))
        {
            assert_eq!(
                (sample.sequence, sample.sampled_at_boot_ms),
                (event.0, event.1)
            );
            let gap = sample
                .gap_before
                .as_ref()
                .map(|gap| (gap.first_sequence, gap.last_sequence));
            assert_eq!(gap, (index == 15).then_some((16, 17)));
            match reasons()[index % 8].1 {
                None => assert_eq!(sample.outcome, Outcome::Available(Box::new(quantity))),
                Some(reason) => assert_eq!(sample.outcome, Outcome::Unavailable(reason.into())),
            }
        }
        assert!(batch.clone().encode().unwrap().len() <= 4096);
    }
    let encoded = snapshot.clone().encode().unwrap();
    assert!(
        encoded.len()
            <= conduit_data::resolved_quantity_observation_snapshot_byte_bound().unwrap() as usize
    );
    assert_eq!(
        conduit_data::PreparedQuantityObservationSnapshotInput::prepare()
            .unwrap()
            .decode(&encoded)
            .unwrap(),
        snapshot
    );
}
#[test]
fn ordered_chunk_boundary_and_explicit_tail_count_previously_recovered_samples() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let mut view = adapter(&p, &contract, 1, 2);
    view.read_next(&chunk(
        &p,
        &contract,
        &[(1, 4, D::Available), (2, 4, D::Checksum)],
    ))
    .unwrap();
    view.read_next(&chunk(
        &p,
        &contract,
        &[(7, 88, D::Available), (8, 101, D::ProviderFailed)],
    ))
    .unwrap();
    for batch in view.snapshot().unwrap().batches.iter() {
        assert_eq!(batch.samples_outside_view, 3);
        assert_eq!(batch.observations.len(), 1);
        let sample = &batch.observations.as_slice()[0];
        assert_eq!((sample.sequence, sample.sampled_at_boot_ms), (8, 101));
        assert!(sample.gap_before.is_none());
        assert_eq!(
            sample.outcome,
            Outcome::Unavailable("provider-failed".into())
        );
    }
    // Independently prepared full latest chunk preserves the known inter-chunk gap.
    let mut full = adapter(&p, &contract, 2, 2);
    full.read_next(&chunk(
        &p,
        &contract,
        &[(1, 4, D::Available), (2, 4, D::Checksum)],
    ))
    .unwrap();
    full.read_next(&chunk(
        &p,
        &contract,
        &[(7, 88, D::Available), (8, 101, D::ProviderFailed)],
    ))
    .unwrap();
    let snapshot = full.snapshot().unwrap();
    let first = &snapshot.batches.as_slice()[0].observations.as_slice()[0];
    let gap = first.gap_before.as_ref().unwrap();
    assert_eq!((gap.first_sequence, gap.last_sequence), (3, 6));
}
#[test]
fn corrupt_foreign_replayed_and_reversed_chunks_preserve_the_entire_previous_view() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let mut view = adapter(&p, &contract, 16, 8);
    let original = chunk(
        &p,
        &contract,
        &[(1, 10, D::Available), (3, 14, D::Checksum)],
    );
    view.read_next(&original).unwrap();
    let before = view.snapshot().unwrap().encode().unwrap();
    let mut foreign = Producer::new();
    foreign.boot = "reported/foreign".into();
    let next = chunk(&p, &contract, &[(4, 15, D::Available)]);
    let corrupt = next.replacen("\"record_bytes\":21", "\"record_bytes\":20", 1);
    let mut bad_hex = next.clone();
    let records = bad_hex.find("\"records\":\"").unwrap() + "\"records\":\"".len();
    bad_hex.replace_range(records..records + 2, "zz");
    for refused in [
        original,
        chunk(&foreign, &contract, &[(4, 15, D::Available)]),
        chunk(&p, &contract, &[(4, 13, D::Available)]),
        corrupt,
        bad_hex,
        next[..next.len() - 1].into(),
    ] {
        assert!(view.read_next(&refused).is_err());
        assert_eq!(view.snapshot().unwrap().encode().unwrap(), before);
    }
    view.read_next(&next).unwrap();
    assert_ne!(view.snapshot().unwrap().encode().unwrap(), before);
}
#[test]
fn explicit_view_identity_and_preparation_bounds_refuse_unsupported_inputs() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let [temperature, humidity] = channels();
    for tail in [0, 17, 64] {
        assert_eq!(
            Adapter::prepare(p.context(), &contract, 1, temperature, humidity, tail).err(),
            Some(Refusal::Window)
        );
    }
    let alias = Channel {
        channel_identity: temperature.source_identity,
        ..humidity
    };
    assert_eq!(
        Adapter::prepare(p.context(), &contract, 1, temperature, alias, 16).err(),
        Some(Refusal::Identity)
    );
    assert!(Adapter::prepare(p.context(), &contract, 0, temperature, humidity, 16).is_err());
    let mut view = adapter(&p, &contract, 16, 1);
    view.read_next(&chunk(&p, &contract, &[(1, 10, D::Available)]))
        .unwrap();
    let retained = view.retained_backing_bytes().unwrap();
    let snapshot = view.snapshot().unwrap();
    let output =
        conduit_data::resolved_quantity_observation_snapshot_backing_bytes(&snapshot).unwrap();
    assert!(retained + output <= Adapter::preparation_backing_bound(&contract).unwrap());
    assert!(retained > 0 && contract.retained_backing_bytes().unwrap() > 0);
    let _checked: ResolvedQuantityObservationSnapshot = snapshot;
}

#[test]
fn dense_gaps_refuse_extent_without_implicitly_shrinking_the_selected_tail() {
    let p = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    // Twenty retained events select sixteen visible events, each with a known
    // preceding gap. The metadata must not be dropped to fit the4096B profile.
    let events = (0..20)
        .map(|n| (2 * n + 1, n * n * 7, reasons()[n as usize % 8].0))
        .collect::<Vec<_>>();
    let content = chunk(&p, &contract, &events);
    let mut view = adapter(&p, &contract, 16, 1);
    view.read_next(&content).unwrap();
    let backing = view.retained_backing_bytes();
    for _ in 0..3 {
        assert_eq!(view.snapshot().err(), Some(Refusal::Extent));
        assert_eq!(view.retained_backing_bytes(), backing);
    }
    // A smaller view requires a distinct explicit preparation, not a hidden
    // reduction of the refused sixteen-sample window.
    let mut smaller = adapter(&p, &contract, 1, 1);
    smaller.read_next(&content).unwrap();
    for batch in smaller.snapshot().unwrap().batches.iter() {
        assert_eq!(batch.observations.len(), 1);
        assert_eq!(batch.samples_outside_view, 19);
        assert_eq!(batch.observations.as_slice()[0].sequence, 39);
    }
    assert_eq!(view.snapshot().err(), Some(Refusal::Extent));
}
