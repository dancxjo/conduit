use conduit_time::*;

fn gap() -> HistoricalRetentionGap {
    HistoricalRetentionGap {
        first_sequence: 40,
        last_sequence: 44,
        entries: 3,
        referenced_bytes: 1_024,
    }
}

#[test]
fn retention_gap_round_trip_preserves_exact_missing_history_evidence() {
    let mut encoded = [0; HISTORICAL_RETENTION_GAP_BYTES];
    let length = encode_historical_retention_gap_into(gap(), &mut encoded).unwrap();
    assert_eq!(length, HISTORICAL_RETENTION_GAP_BYTES);
    assert_eq!(decode_historical_retention_gap(&encoded), Ok(gap()));
}

#[test]
fn invalid_pressure_version_truncation_and_trailing_bytes_remain_distinct() {
    let mut encoded = [0; HISTORICAL_RETENTION_GAP_BYTES];
    assert_eq!(
        encode_historical_retention_gap_into(gap(), &mut encoded[..36]),
        Err(HistoricalRetentionGapCodecRefusal::OutputTooSmall)
    );
    assert_eq!(
        encode_historical_retention_gap_into(
            HistoricalRetentionGap {
                entries: 0,
                ..gap()
            },
            &mut encoded,
        ),
        Err(HistoricalRetentionGapCodecRefusal::InvalidGap)
    );
    encode_historical_retention_gap_into(gap(), &mut encoded).unwrap();
    assert_eq!(
        decode_historical_retention_gap(&encoded[..36]),
        Err(HistoricalRetentionGapCodecRefusal::Truncated)
    );
    let mut malformed = encoded;
    malformed[0] ^= 1;
    assert_eq!(
        decode_historical_retention_gap(&malformed),
        Err(HistoricalRetentionGapCodecRefusal::InvalidMagic)
    );
    let mut malformed = encoded;
    malformed[4] += 1;
    assert_eq!(
        decode_historical_retention_gap(&malformed),
        Err(HistoricalRetentionGapCodecRefusal::UnsupportedVersion)
    );
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_historical_retention_gap(&trailing),
        Err(HistoricalRetentionGapCodecRefusal::TrailingBytes)
    );
}
