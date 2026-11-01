use super::continuing::native::{CompletedScope, LiveFore};
use super::*;
use conduit_environment_measurement::{
    source_observation::{
        PreparedObservationCodec, SourceFailure, SourceObservation, SourceSample,
    },
    EnvironmentalReading,
};
fn selection(plan: &str) -> Arc<SourceSelection> {
    Arc::new(SourceSelection {
        attachment_id: "green".into(),
        host_id: "host/collector".into(),
        boot_id: "boot/collector".into(),
        capability_id: "cap/source".into(),
        provider_instance_id: "session/green".into(),
        provider_generation: 1,
        plan_id: plan.into(),
        fragment_id: "fragment/source".into(),
        placement_id: "placement/source".into(),
        configuration_id: "configuration/green".into(),
        firmware_sha256: [1; 32],
    })
}
fn message(
    selection: Arc<SourceSelection>,
    sequence: u64,
    observed: u64,
    counter: Option<u64>,
) -> LiveFore {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let outcome = counter.map_or(
        SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        |source_counter| {
            SourceOutcome::Available(SourceSample {
                reading: EnvironmentalReading::centi_celsius_centi_pascal(
                    2200,
                    10000000,
                    Some(50 * 1024),
                )
                .unwrap(),
                source_counter,
                reported_source_ms: Some(source_counter),
            })
        },
    );
    let canonical = codec
        .encode(SourceObservation {
            observed_at_host_boot_ms: observed,
            outcome,
        })
        .unwrap();
    let mut bytes = [0; MAXIMUM_BYTES];
    bytes[..canonical.len()].copy_from_slice(canonical);
    let active = conduit_core::bind_active_play(
        &selection.plan_id,
        &selection.host_id,
        &selection.boot_id,
        1,
    );
    LiveFore {
        scope: Arc::new(CompletedScope {
            selection,
            native_play: active,
        }),
        sequence,
        length: canonical.len(),
        bytes,
    }
}
#[test]
fn last_good_survives_twenty_unavailable_outcomes_and_new_child_play() {
    let first = selection("plan/first");
    let mut cache = EnvironmentalFace::prepare().unwrap();
    cache.select(first.clone(), None).unwrap();
    let good = message(first.clone(), 0, 10, Some(1));
    cache.accept(&good, 10).unwrap();
    for n in 1..=20 {
        cache
            .accept(&message(first.clone(), n, 10 + n, None), 10 + n)
            .unwrap();
    }
    let rows = cache.sources[0].records();
    assert_eq!(rows.len(), 16);
    assert_eq!(rows[0].observation, &good.bytes[..good.length]);
    let next = selection("plan/next");
    cache.select(next.clone(), None).unwrap();
    assert!(cache
        .accept(&message(next.clone(), 1, 32, Some(2)), 32)
        .is_err());
    assert!(cache
        .accept(&message(next.clone(), 0, 32, Some(1)), 32)
        .is_err());
    cache.accept(&message(next, 0, 32, Some(2)), 32).unwrap();
    assert_eq!(cache.sources[0].accepted, 22);
    assert_eq!(
        cache
            .sources
            .iter()
            .filter(|s| s.selection.is_some())
            .count(),
        1
    );
}
#[test]
fn wrong_scope_clock_and_order_leave_canonical_storage_unchanged() {
    let selected = selection("plan/source");
    let mut cache = EnvironmentalFace::prepare().unwrap();
    cache.select(selected.clone(), None).unwrap();
    cache
        .accept(&message(selected.clone(), 0, 10, Some(1)), 10)
        .unwrap();
    let before = cache.sources[0].bytes;
    assert!(cache
        .accept(&message(selection("plan/unselected"), 1, 11, Some(2)), 11)
        .is_err());
    assert!(cache
        .accept(&message(selected.clone(), 1, 12, Some(2)), 11)
        .is_err());
    assert!(cache
        .accept(&message(selected, 0, 11, Some(2)), 11)
        .is_err());
    assert_eq!(cache.sources[0].bytes, before);
    assert_eq!(cache.sources[0].accepted, 1);
}

#[test]
fn descriptive_placement_changes_without_rewriting_source_or_observations() {
    let selected = selection("plan/source");
    let mut cache = EnvironmentalFace::prepare().unwrap();
    cache
        .select(selected.clone(), Some("Forebrain room".into()))
        .unwrap();
    cache
        .accept(&message(selected.clone(), 0, 10, Some(1)), 10)
        .unwrap();
    let subject = cache.sources[0].subject.clone();
    let canonical = cache.sources[0].bytes;
    cache
        .select(selected.clone(), Some("Greenhouse".into()))
        .unwrap();
    assert_eq!(cache.sources[0].subject, subject);
    assert_eq!(cache.sources[0].bytes, canonical);
    assert!(cache.sources[0].selection.as_deref() == Some(selected.as_ref()));
    let fragment = cache
        .contribution(
            "host/collector",
            "boot/collector",
            10,
            FaceContributionBasis {
                checked_plot_id: "plot/source".into(),
                plan_id: "plan/source".into(),
                active_play_id: "play/source".into(),
                required_interaction_context: None,
            },
        )
        .unwrap()
        .unwrap();
    assert!(fragment.properties.iter().any(|p| p.name == "placement"
        && p.value == conduit_face::FacePropertyValue::Text("Greenhouse".into())));
    assert!(cache
        .select(selected, Some("bad\nplacement".into()))
        .is_err());
    assert_eq!(cache.sources[0].placement.as_deref(), Some("Greenhouse"));
}
