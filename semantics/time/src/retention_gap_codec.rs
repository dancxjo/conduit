//! Finite canonical value for an explicit historical retention gap.

use crate::HistoricalRetentionGap;

pub const HISTORICAL_RETENTION_GAP_WIRE_VERSION: u8 = 1;
pub const HISTORICAL_RETENTION_GAP_BYTES: usize = 37;
const MAGIC: [u8; 4] = *b"HGAP";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HistoricalRetentionGapCodecRefusal {
    InvalidGap,
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    TrailingBytes,
}

pub fn encode_historical_retention_gap_into(
    gap: HistoricalRetentionGap,
    output: &mut [u8],
) -> Result<usize, HistoricalRetentionGapCodecRefusal> {
    validate(gap)?;
    if output.len() < HISTORICAL_RETENTION_GAP_BYTES {
        return Err(HistoricalRetentionGapCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = HISTORICAL_RETENTION_GAP_WIRE_VERSION;
    let mut cursor = 5;
    for value in [
        gap.first_sequence,
        gap.last_sequence,
        gap.entries,
        gap.referenced_bytes,
    ] {
        output[cursor..cursor + 8].copy_from_slice(&value.to_le_bytes());
        cursor += 8;
    }
    Ok(cursor)
}

pub fn decode_historical_retention_gap(
    encoded: &[u8],
) -> Result<HistoricalRetentionGap, HistoricalRetentionGapCodecRefusal> {
    if encoded.len() < HISTORICAL_RETENTION_GAP_BYTES {
        return Err(HistoricalRetentionGapCodecRefusal::Truncated);
    }
    if encoded[..4] != MAGIC {
        return Err(HistoricalRetentionGapCodecRefusal::InvalidMagic);
    }
    if encoded[4] != HISTORICAL_RETENTION_GAP_WIRE_VERSION {
        return Err(HistoricalRetentionGapCodecRefusal::UnsupportedVersion);
    }
    if encoded.len() != HISTORICAL_RETENTION_GAP_BYTES {
        return Err(HistoricalRetentionGapCodecRefusal::TrailingBytes);
    }
    let gap = HistoricalRetentionGap {
        first_sequence: read_u64(encoded, 5),
        last_sequence: read_u64(encoded, 13),
        entries: read_u64(encoded, 21),
        referenced_bytes: read_u64(encoded, 29),
    };
    validate(gap)?;
    Ok(gap)
}

fn validate(gap: HistoricalRetentionGap) -> Result<(), HistoricalRetentionGapCodecRefusal> {
    let span = gap
        .last_sequence
        .checked_sub(gap.first_sequence)
        .and_then(|span| span.checked_add(1));
    if gap.entries == 0 || span.is_none_or(|span| gap.entries > span) {
        return Err(HistoricalRetentionGapCodecRefusal::InvalidGap);
    }
    Ok(())
}

fn read_u64(encoded: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(
        encoded[start..start + 8]
            .try_into()
            .expect("the checked retention-gap slice is exact"),
    )
}
