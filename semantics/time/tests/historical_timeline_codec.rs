use conduit_core::{
    kind_id, semantic_digest, BoundedResourceRef, ResourceClassId, ResourceExtent,
    ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant,
    TemporalScale,
};
use conduit_time::*;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/field".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn value(seed: u8, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id("observation/light@1"),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed + 1; 32]),
            expires_at: None,
        },
    }
}

fn timeline() -> BoundedHistoricalTimeline {
    let mut timeline = BoundedHistoricalTimeline::new(
        kind_id("observation/light@1"),
        "clock/field",
        TemporalScale::Milliseconds,
        2,
        10,
        HistoricalOverflowPolicy::EvictOldestWithGap,
        7,
    )
    .unwrap();
    timeline
        .append(
            "event/old".into(),
            at(100),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 6),
        )
        .unwrap();
    timeline
        .append(
            "event/kept".into(),
            at(110),
            HistoricalEntryOrigin::OperatorAuthored,
            value(3, 6),
        )
        .unwrap();
    timeline
}

fn reseal(encoded: &mut [u8]) {
    let payload_length = encoded.len() - 32;
    let digest = semantic_digest("history/timeline-snapshot@1", &encoded[..payload_length]);
    encoded[payload_length..].copy_from_slice(&digest);
}

fn overflow_policy_offset(encoded: &[u8]) -> usize {
    let profile_length = usize::from(u16::from_le_bytes([encoded[5], encoded[6]]));
    let clock_length_offset = 7 + profile_length;
    let clock_length = usize::from(u16::from_le_bytes([
        encoded[clock_length_offset],
        encoded[clock_length_offset + 1],
    ]));
    clock_length_offset + 2 + clock_length + 1 + 2 + 8
}

#[test]
fn complete_timeline_state_round_trips_through_caller_storage() {
    let source = timeline();
    let mut encoded = vec![0_u8; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&source, &mut encoded).unwrap();
    let restored = decode_historical_timeline(&encoded[..length]).unwrap();

    assert_eq!(restored.len(), source.len());
    assert_eq!(restored.referenced_bytes(), source.referenced_bytes());
    assert_eq!(restored.retention_gap(), source.retention_gap());
    assert_eq!(restored.clear_revision(), source.clear_revision());
    assert_eq!(restored.entry(0), source.entry(0));
    assert_eq!(restored.replay_metadata(), source.replay_metadata());
    assert_eq!(
        restored.retention_gap(),
        Some(HistoricalRetentionGap {
            first_sequence: 7,
            last_sequence: 7,
            entries: 1,
            referenced_bytes: 6,
        })
    );
}

#[test]
fn maximum_entry_count_with_maximal_identities_fits_the_snapshot_envelope() {
    let profile = "p".repeat(conduit_core::MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES);
    let clock = "c".repeat(conduit_core::MAXIMUM_TEMPORAL_IDENTITY_BYTES);
    let mut source = BoundedHistoricalTimeline::new(
        kind_id(&profile),
        &clock,
        TemporalScale::Nanoseconds,
        MAXIMUM_HISTORICAL_TIMELINE_ENTRIES,
        MAXIMUM_HISTORICAL_TIMELINE_ENTRIES as u64,
        HistoricalOverflowPolicy::Refuse,
        0,
    )
    .unwrap();
    for index in 0..MAXIMUM_HISTORICAL_TIMELINE_ENTRIES {
        let seed = u8::try_from(index + 1).unwrap();
        source
            .append(
                format!("event/{index:03}/{}", "i".repeat(118)),
                TemporalInstant {
                    ticks: index as u64,
                    scale: TemporalScale::Nanoseconds,
                    clock_basis: clock.clone(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                },
                HistoricalEntryOrigin::MachineObservation,
                BoundedResourceRef {
                    identity: ResourceSemanticIdentity::from_digest([seed; 32]),
                    content_profile: kind_id(&profile),
                    access_class: ResourceClassId::from("r"),
                    extent: ResourceExtent {
                        bytes: 1,
                        items: Some(1),
                    },
                    lifetime: ResourceLifetime {
                        version: ResourceVersionIdentity::from_digest([seed.wrapping_add(64); 32]),
                        expires_at: Some(TemporalInstant {
                            ticks: index as u64 + 1,
                            scale: TemporalScale::Nanoseconds,
                            clock_basis: clock.clone(),
                            resolution_ticks: 1,
                            uncertainty_ticks: 0,
                        }),
                    },
                },
            )
            .unwrap();
    }

    let mut encoded = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&source, &mut encoded).unwrap();
    assert!(length <= MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES);
    assert_eq!(
        decode_historical_timeline(&encoded[..length])
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn output_truncation_magic_version_and_trailing_bytes_are_distinct() {
    let source = timeline();
    let mut encoded = vec![0_u8; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&source, &mut encoded).unwrap();
    assert_eq!(
        encode_historical_timeline_into(&source, &mut encoded[..length - 1]),
        Err(HistoricalTimelineCodecRefusal::OutputTooSmall)
    );
    assert!(matches!(
        decode_historical_timeline(&encoded[..4]),
        Err(HistoricalTimelineCodecRefusal::Truncated)
    ));
    let mut malformed = encoded[..length].to_vec();
    malformed[0] ^= 1;
    reseal(&mut malformed);
    assert!(matches!(
        decode_historical_timeline(&malformed),
        Err(HistoricalTimelineCodecRefusal::InvalidMagic)
    ));
    malformed = encoded[..length].to_vec();
    malformed[4] += 1;
    reseal(&mut malformed);
    assert!(matches!(
        decode_historical_timeline(&malformed),
        Err(HistoricalTimelineCodecRefusal::UnsupportedVersion)
    ));
    malformed = encoded[..length].to_vec();
    let digest = malformed.split_off(malformed.len() - 32);
    malformed.push(0);
    malformed.extend_from_slice(&digest);
    reseal(&mut malformed);
    assert!(matches!(
        decode_historical_timeline(&malformed),
        Err(HistoricalTimelineCodecRefusal::TrailingBytes)
    ));
}

#[test]
fn corrupted_resource_and_invalid_snapshot_sequence_fail_closed() {
    let source = timeline();
    let mut encoded = vec![0_u8; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&source, &mut encoded).unwrap();
    encoded.truncate(length);

    let last = encoded.len() - 1;
    encoded[last] ^= 1;
    assert!(matches!(
        decode_historical_timeline(&encoded),
        Err(HistoricalTimelineCodecRefusal::Integrity)
    ));

    let mut encoded = vec![0_u8; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&timeline(), &mut encoded).unwrap();
    encoded.truncate(length);
    let overflow = overflow_policy_offset(&encoded);
    encoded[overflow] = 0;
    reseal(&mut encoded);
    assert!(matches!(
        decode_historical_timeline(&encoded),
        Err(HistoricalTimelineCodecRefusal::Timeline(
            HistoricalTimelineRefusal::InvalidSnapshot
        ))
    ));

    let mut source = timeline();
    source.clear().unwrap();
    source
        .append(
            "event/new".into(),
            at(200),
            HistoricalEntryOrigin::MachineObservation,
            value(7, 2),
        )
        .unwrap();
    let mut encoded = vec![0_u8; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&source, &mut encoded).unwrap();
    let restored = decode_historical_timeline(&encoded[..length]).unwrap();
    assert_eq!(restored.clear_revision(), 1);
    assert_eq!(restored.entry(0).unwrap().sequence, 9);
}
