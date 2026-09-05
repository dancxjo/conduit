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
        clock_basis: "clock/history".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn value(seed: u8) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id("observation/light@1"),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed + 1; 32]),
            expires_at: None,
        },
    }
}

fn append(identity: &str, ticks: u64, seed: u8) -> HistoricalTimelineCommand {
    HistoricalTimelineCommand::Append {
        identity: identity.into(),
        event_time: at(ticks),
        origin: HistoricalEntryOrigin::OperatorAuthored,
        value: value(seed),
    }
}

fn timeline() -> BoundedHistoricalTimeline {
    BoundedHistoricalTimeline::new(
        kind_id("observation/light@1"),
        "clock/history",
        TemporalScale::Milliseconds,
        4,
        16,
        HistoricalOverflowPolicy::Refuse,
        7,
    )
    .unwrap()
}

fn encode(command: &HistoricalTimelineCommand) -> Vec<u8> {
    let mut encoded = vec![0; MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES];
    let length = encode_historical_timeline_command_into(command, &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

fn reseal(encoded: &mut [u8]) {
    let payload = encoded.len() - 32;
    let digest = semantic_digest(HISTORICAL_TIMELINE_COMMAND_INFO_ID, &encoded[..payload]);
    encoded[payload..].copy_from_slice(&digest);
}

#[test]
fn append_remove_and_clear_commands_round_trip_and_mutate_explicitly() {
    let commands = [
        append("event/amber", 100, 1),
        HistoricalTimelineCommand::Remove { sequence: 7 },
        HistoricalTimelineCommand::Clear,
    ];
    for command in &commands {
        assert_eq!(
            decode_historical_timeline_command(&encode(command)),
            Ok(command.clone())
        );
    }

    let mut history = timeline();
    assert_eq!(
        history.apply(commands[0].clone()).unwrap(),
        HistoricalTimelineOutcome::Appended { sequence: 7 }
    );
    let HistoricalTimelineOutcome::Removed(removed) = history.apply(commands[1].clone()).unwrap()
    else {
        panic!("remove must return the exact removed entry");
    };
    assert_eq!(removed.identity, "event/amber");
    history.apply(append("event/blue", 110, 3)).unwrap();
    assert_eq!(
        history.apply(commands[2].clone()).unwrap(),
        HistoricalTimelineOutcome::Cleared { revision: 1 }
    );
    assert!(history.is_empty());
}

#[test]
fn command_codec_failures_remain_bounded_and_distinct() {
    let command = append("event/amber", 100, 1);
    let encoded = encode(&command);
    let mut short = vec![0; encoded.len() - 1];
    assert_eq!(
        encode_historical_timeline_command_into(&command, &mut short),
        Err(HistoricalTimelineCommandCodecRefusal::OutputTooSmall)
    );
    assert_eq!(
        decode_historical_timeline_command(&encoded[..31]),
        Err(HistoricalTimelineCommandCodecRefusal::Truncated)
    );

    let mut corrupt = encoded.clone();
    corrupt[8] ^= 1;
    assert_eq!(
        decode_historical_timeline_command(&corrupt),
        Err(HistoricalTimelineCommandCodecRefusal::Integrity)
    );
    let mut invalid = encoded;
    invalid[5] = 9;
    reseal(&mut invalid);
    assert_eq!(
        decode_historical_timeline_command(&invalid),
        Err(HistoricalTimelineCommandCodecRefusal::InvalidCommand)
    );
}

#[test]
fn decoded_command_still_obeys_the_timeline_type_and_order_contract() {
    let mut history = timeline();
    let first = decode_historical_timeline_command(&encode(&append("event/one", 100, 1))).unwrap();
    history.apply(first).unwrap();

    let mut wrong = append("event/wrong", 110, 3);
    let HistoricalTimelineCommand::Append { value, .. } = &mut wrong else {
        unreachable!();
    };
    value.content_profile = kind_id("observation/sound@1");
    let decoded = decode_historical_timeline_command(&encode(&wrong)).unwrap();
    assert_eq!(
        history.apply(decoded),
        Err(HistoricalTimelineRefusal::WrongValueProfile)
    );

    let reordered =
        decode_historical_timeline_command(&encode(&append("event/old", 99, 5))).unwrap();
    assert_eq!(
        history.apply(reordered),
        Err(HistoricalTimelineRefusal::ReorderedEventTime)
    );
    assert_eq!(history.len(), 1);
}
