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
}
