use conduit_time::{
    decode_playback_tick, encode_playback_tick_into, PlaybackTickCodecRefusal, PLAYBACK_TICK_BYTES,
};

#[test]
fn playback_ticks_round_trip_without_becoming_historical_time() {
    let mut encoded = [0; PLAYBACK_TICK_BYTES];
    assert_eq!(
        encode_playback_tick_into(9_000, &mut encoded),
        Ok(PLAYBACK_TICK_BYTES)
    );
    assert_eq!(decode_playback_tick(&encoded), Ok(9_000));
}

#[test]
fn playback_tick_wire_failures_remain_distinct() {
    assert_eq!(
        encode_playback_tick_into(1, &mut [0; PLAYBACK_TICK_BYTES - 1]),
        Err(PlaybackTickCodecRefusal::OutputTooSmall)
    );
    let mut encoded = [0; PLAYBACK_TICK_BYTES];
    encode_playback_tick_into(1, &mut encoded).unwrap();
    assert_eq!(
        decode_playback_tick(&encoded[..PLAYBACK_TICK_BYTES - 1]),
        Err(PlaybackTickCodecRefusal::Truncated)
    );
    let mut bad_magic = encoded;
    bad_magic[0] = b'X';
    assert_eq!(
        decode_playback_tick(&bad_magic),
        Err(PlaybackTickCodecRefusal::InvalidMagic)
    );
    let mut bad_version = encoded;
    bad_version[4] = 2;
    assert_eq!(
        decode_playback_tick(&bad_version),
        Err(PlaybackTickCodecRefusal::UnsupportedVersion)
    );
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_playback_tick(&trailing),
        Err(PlaybackTickCodecRefusal::TrailingBytes)
    );
}
