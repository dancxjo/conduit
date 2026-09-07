//! Exact finite wire value for the replay controller's distinct playback clock.

pub const PLAYBACK_TICK_INFO_ID: &str = "time/playback-tick@1";
pub const PLAYBACK_TICK_BYTES: usize = 13;

const MAGIC: [u8; 4] = *b"PBTK";
const VERSION: u8 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PlaybackTickCodecRefusal {
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    TrailingBytes,
}

pub fn encode_playback_tick_into(
    ticks: u64,
    output: &mut [u8],
) -> Result<usize, PlaybackTickCodecRefusal> {
    if output.len() < PLAYBACK_TICK_BYTES {
        return Err(PlaybackTickCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = VERSION;
    output[5..13].copy_from_slice(&ticks.to_le_bytes());
    Ok(PLAYBACK_TICK_BYTES)
}

pub fn decode_playback_tick(encoded: &[u8]) -> Result<u64, PlaybackTickCodecRefusal> {
    if encoded.len() < PLAYBACK_TICK_BYTES {
        return Err(PlaybackTickCodecRefusal::Truncated);
    }
    if encoded[..4] != MAGIC {
        return Err(PlaybackTickCodecRefusal::InvalidMagic);
    }
    if encoded[4] != VERSION {
        return Err(PlaybackTickCodecRefusal::UnsupportedVersion);
    }
    if encoded.len() != PLAYBACK_TICK_BYTES {
        return Err(PlaybackTickCodecRefusal::TrailingBytes);
    }
    Ok(u64::from_le_bytes(
        encoded[5..13]
            .try_into()
            .expect("the exact playback tick length was checked"),
    ))
}
