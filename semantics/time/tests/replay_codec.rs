use conduit_time::*;

fn entries() -> Vec<HistoricalReplayEntry> {
    vec![
        HistoricalReplayEntry {
            identity: "observation/one".into(),
            event_ticks: 700,
        },
        HistoricalReplayEntry {
            identity: "observation/two".into(),
            event_ticks: 725,
        },
    ]
}

#[test]
fn timeline_round_trips_exact_history_through_caller_storage() {
    let source = entries();
    let mut encoded = [0_u8; MAXIMUM_REPLAY_TIMELINE_BYTES];
    let length = encode_replay_timeline_into(&source, &mut encoded).unwrap();
    assert!(length < encoded.len());
    assert_eq!(decode_replay_timeline(&encoded[..length]), Ok(source));
}

#[test]
fn output_truncation_magic_version_utf8_and_trailing_fail_distinctly() {
    let source = entries();
    let mut encoded = [0_u8; MAXIMUM_REPLAY_TIMELINE_BYTES];
    let length = encode_replay_timeline_into(&source, &mut encoded).unwrap();
    assert_eq!(
        encode_replay_timeline_into(&source, &mut encoded[..length - 1]),
        Err(ReplayTimelineCodecRefusal::OutputTooSmall)
    );
    assert_eq!(
        decode_replay_timeline(&encoded[..6]),
        Err(ReplayTimelineCodecRefusal::Truncated)
    );
    let mut malformed = encoded[..length].to_vec();
    malformed[0] ^= 1;
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::InvalidMagic)
    );
    malformed = encoded[..length].to_vec();
    malformed[4] += 1;
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::UnsupportedVersion)
    );
    malformed = encoded[..length].to_vec();
    malformed[9] = 0xff;
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::InvalidUtf8)
    );
    malformed = encoded[..length].to_vec();
    malformed.push(0);
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::TrailingBytes)
    );

    malformed = encoded[..length].to_vec();
    malformed.truncate(9);
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::Truncated)
    );

    malformed = encoded[..length].to_vec();
    let first_identity_length = usize::from(u16::from_le_bytes([malformed[7], malformed[8]]));
    malformed.truncate(9 + first_identity_length + 7);
    assert_eq!(
        decode_replay_timeline(&malformed),
        Err(ReplayTimelineCodecRefusal::Truncated)
    );
}

#[test]
fn empty_duplicate_reordered_and_oversized_timelines_refuse() {
    let mut output = [0_u8; MAXIMUM_REPLAY_TIMELINE_BYTES];
    assert_eq!(
        encode_replay_timeline_into(&[], &mut output),
        Err(ReplayTimelineCodecRefusal::EmptyTimeline)
    );
    let duplicate = vec![
        HistoricalReplayEntry {
            identity: "same".into(),
            event_ticks: 1,
        },
        HistoricalReplayEntry {
            identity: "same".into(),
            event_ticks: 2,
        },
    ];
    assert_eq!(
        encode_replay_timeline_into(&duplicate, &mut output),
        Err(ReplayTimelineCodecRefusal::DuplicateIdentity)
    );
    let reordered = vec![
        HistoricalReplayEntry {
            identity: "one".into(),
            event_ticks: 2,
        },
        HistoricalReplayEntry {
            identity: "two".into(),
            event_ticks: 1,
        },
    ];
    assert_eq!(
        encode_replay_timeline_into(&reordered, &mut output),
        Err(ReplayTimelineCodecRefusal::ReorderedHistoricalTime)
    );
    let too_many = (0..=MAXIMUM_REPLAY_ENTRIES)
        .map(|index| HistoricalReplayEntry {
            identity: format!("event/{index}"),
            event_ticks: index as u64,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        encode_replay_timeline_into(&too_many, &mut output),
        Err(ReplayTimelineCodecRefusal::TooManyEntries)
    );

    let empty_identity = vec![HistoricalReplayEntry {
        identity: String::new(),
        event_ticks: 0,
    }];
    assert_eq!(
        encode_replay_timeline_into(&empty_identity, &mut output),
        Err(ReplayTimelineCodecRefusal::EmptyIdentity)
    );
    let long_identity = vec![HistoricalReplayEntry {
        identity: "x".repeat(MAXIMUM_REPLAY_IDENTITY_BYTES + 1),
        event_ticks: 0,
    }];
    assert_eq!(
        encode_replay_timeline_into(&long_identity, &mut output),
        Err(ReplayTimelineCodecRefusal::IdentityTooLong)
    );

    let mut declared_too_many = vec![0_u8; 7];
    declared_too_many[..4].copy_from_slice(b"CRTL");
    declared_too_many[4] = REPLAY_TIMELINE_WIRE_VERSION;
    declared_too_many[5..7].copy_from_slice(&((MAXIMUM_REPLAY_ENTRIES + 1) as u16).to_le_bytes());
    assert_eq!(
        decode_replay_timeline(&declared_too_many),
        Err(ReplayTimelineCodecRefusal::TooManyEntries)
    );

    let mut declared_empty_identity = vec![0_u8; 9];
    declared_empty_identity[..4].copy_from_slice(b"CRTL");
    declared_empty_identity[4] = REPLAY_TIMELINE_WIRE_VERSION;
    declared_empty_identity[5..7].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_replay_timeline(&declared_empty_identity),
        Err(ReplayTimelineCodecRefusal::EmptyIdentity)
    );
}
