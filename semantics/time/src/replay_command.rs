//! Finite wire value for explicit replay-control operations.

pub const MAXIMUM_REPLAY_COMMAND_BYTES: usize = 8;

const MAGIC: [u8; 4] = *b"RCTL";
const VERSION: u8 = 1;

use crate::ReplayCommand;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReplayCommandCodecRefusal {
    OutputTooSmall,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    UnknownCommand,
    TrailingBytes,
}

pub fn encode_replay_command_into(
    command: ReplayCommand,
    output: &mut [u8],
) -> Result<usize, ReplayCommandCodecRefusal> {
    let length = if matches!(&command, ReplayCommand::Fail(_)) {
        8
    } else {
        6
    };
    if output.len() < length {
        return Err(ReplayCommandCodecRefusal::OutputTooSmall);
    }
    output[..4].copy_from_slice(&MAGIC);
    output[4] = VERSION;
    output[5] = match &command {
        ReplayCommand::Start => 0,
        ReplayCommand::Stop => 1,
        ReplayCommand::Pause => 2,
        ReplayCommand::Resume => 3,
        ReplayCommand::Restart => 4,
        ReplayCommand::Step => 5,
        ReplayCommand::Fail(_) => 6,
    };
    if let ReplayCommand::Fail(failure) = command {
        output[6..8].copy_from_slice(&failure.code().to_le_bytes());
    }
    Ok(length)
}

pub fn decode_replay_command(encoded: &[u8]) -> Result<ReplayCommand, ReplayCommandCodecRefusal> {
    if encoded.len() < 6 {
        return Err(ReplayCommandCodecRefusal::Truncated);
    }
    if encoded[..4] != MAGIC {
        return Err(ReplayCommandCodecRefusal::InvalidMagic);
    }
    if encoded[4] != VERSION {
        return Err(ReplayCommandCodecRefusal::UnsupportedVersion);
    }
    let (command, expected) = match encoded[5] {
        0 => (ReplayCommand::Start, 6),
        1 => (ReplayCommand::Stop, 6),
        2 => (ReplayCommand::Pause, 6),
        3 => (ReplayCommand::Resume, 6),
        4 => (ReplayCommand::Restart, 6),
        5 => (ReplayCommand::Step, 6),
        6 => {
            if encoded.len() < 8 {
                return Err(ReplayCommandCodecRefusal::Truncated);
            }
            (
                ReplayCommand::fail(u16::from_le_bytes([encoded[6], encoded[7]]))
                    .expect("every U16 is a valid replay failure code"),
                8,
            )
        }
        _ => return Err(ReplayCommandCodecRefusal::UnknownCommand),
    };
    if encoded.len() != expected {
        return Err(ReplayCommandCodecRefusal::TrailingBytes);
    }
    Ok(command)
}
