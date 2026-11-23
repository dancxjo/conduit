//! One finite correlated Create observation session.

use crate::{
    decode_sensor_packet, encode_mode, encode_pause_stream, encode_sensor_stream_pair,
    encode_start, lower_charging_sources, lower_group_zero, read_expected_stream_frame,
    write_command, CreateChargingSources, CreateGroupZeroObservation, CreateOiFailure,
    CreateOiModeRequest, CreateOiPacket, CreateSensorLoweringError, CreateUartProvider,
    EncodedOiCommand, CREATE_CHARGING_SOURCES_PACKET_ID, CREATE_GROUP_ZERO_PACKET_ID,
    CREATE_OI_MAX_COMMAND_BYTES, STREAM_HEADER,
};

const GROUP_ZERO_BYTES: usize = 26;
const CHARGING_SOURCE_BYTES: usize = 1;
const BUNDLE_PAYLOAD_BYTES: usize = 1 + GROUP_ZERO_BYTES + 1 + CHARGING_SOURCE_BYTES;
pub const CREATE_OBSERVATION_FRAME_BYTES: usize = BUNDLE_PAYLOAD_BYTES + 3;
pub const CREATE_OBSERVATION_MAXIMUM_DISCARDED_BYTES: u16 = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateObservationPacketBundle {
    pub group_zero: CreateOiPacket,
    pub charging_sources: CreateOiPacket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePortableObservation {
    pub group_zero: CreateGroupZeroObservation,
    pub charging_sources: CreateChargingSources,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationFailure {
    Protocol(CreateOiFailure),
    Lowering(CreateSensorLoweringError),
    InvalidState,
}

impl From<CreateOiFailure> for CreateObservationFailure {
    fn from(value: CreateOiFailure) -> Self {
        Self::Protocol(value)
    }
}

impl From<CreateSensorLoweringError> for CreateObservationFailure {
    fn from(value: CreateSensorLoweringError) -> Self {
        Self::Lowering(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationSessionState {
    New,
    Streaming,
    ObservationReady,
    ReadFailed,
    StartFailed,
    Paused,
    PauseFailed,
}

pub struct CreateObservationSession {
    state: CreateObservationSessionState,
    startup_mode: CreateOiModeRequest,
    last_stream_discarded_bytes: Option<u16>,
}

impl CreateObservationSession {
    /// Start in Safe mode for existing observation and actuator workflows.
    pub const fn new() -> Self {
        Self {
            startup_mode: CreateOiModeRequest::Safe,
            ..Self::new_passive()
        }
    }

    /// Request Passive mode for a standalone sensor session, permitting charging.
    /// This changes mode and must not share an active actuator session.
    /// It does not verify that the robot received the request or is charging.
    pub const fn new_passive() -> Self {
        Self {
            state: CreateObservationSessionState::New,
            startup_mode: CreateOiModeRequest::Passive,
            last_stream_discarded_bytes: None,
        }
    }

    pub const fn state(&self) -> CreateObservationSessionState {
        self.state
    }

    pub const fn last_stream_discarded_bytes(&self) -> Option<u16> {
        self.last_stream_discarded_bytes
    }

    pub fn start<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
    ) -> Result<(), CreateObservationFailure> {
        if self.state != CreateObservationSessionState::New {
            return Err(CreateObservationFailure::InvalidState);
        }
        let result = write_command(provider, &encode_start())
            .and_then(|()| {
                match encode_mode(self.startup_mode) {
                    Some(command) => write_command(provider, &command),
                    None => Ok(()), // Start already requests Passive mode.
                }
            })
            .and_then(|()| write_command(provider, &encode_observation_stream()));
        match result {
            Ok(()) => {
                self.state = CreateObservationSessionState::Streaming;
                Ok(())
            }
            Err(failure) => {
                self.state = CreateObservationSessionState::StartFailed;
                Err(failure.into())
            }
        }
    }

    pub fn read<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        deadline_tick: u64,
    ) -> Result<CreatePortableObservation, CreateObservationFailure> {
        if self.state != CreateObservationSessionState::Streaming {
            return Err(CreateObservationFailure::InvalidState);
        }
        let result = read_synchronized_observation_bundle(
            provider,
            deadline_tick,
            CREATE_OBSERVATION_MAXIMUM_DISCARDED_BYTES,
        )
        .map_err(CreateObservationFailure::Protocol)
        .and_then(|read| {
            self.last_stream_discarded_bytes = Some(read.discarded_bytes);
            Ok(CreatePortableObservation {
                group_zero: lower_group_zero(&read.bundle.group_zero)?,
                charging_sources: lower_charging_sources(&read.bundle.charging_sources)?,
            })
        });
        match result {
            Ok(observation) => {
                self.state = CreateObservationSessionState::ObservationReady;
                Ok(observation)
            }
            Err(failure) => {
                self.state = CreateObservationSessionState::ReadFailed;
                Err(failure)
            }
        }
    }

    pub fn pause<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
    ) -> Result<(), CreateObservationFailure> {
        if !matches!(
            self.state,
            CreateObservationSessionState::Streaming
                | CreateObservationSessionState::ObservationReady
                | CreateObservationSessionState::ReadFailed
        ) {
            return Err(CreateObservationFailure::InvalidState);
        }
        match write_command(provider, &encode_pause_stream()) {
            Ok(()) => {
                self.state = CreateObservationSessionState::Paused;
                Ok(())
            }
            Err(failure) => {
                self.state = CreateObservationSessionState::PauseFailed;
                Err(failure.into())
            }
        }
    }
}

impl Default for CreateObservationSession {
    fn default() -> Self {
        Self::new()
    }
}

pub fn encode_observation_stream() -> EncodedOiCommand {
    encode_sensor_stream_pair(
        CREATE_GROUP_ZERO_PACKET_ID,
        CREATE_CHARGING_SOURCES_PACKET_ID,
    )
    .expect("the pinned correlated observation packets are supported")
}

pub fn read_observation_bundle<P: CreateUartProvider>(
    provider: &mut P,
    deadline_tick: u64,
) -> Result<CreateObservationPacketBundle, CreateOiFailure> {
    read_synchronized_observation_bundle(
        provider,
        deadline_tick,
        CREATE_OBSERVATION_MAXIMUM_DISCARDED_BYTES,
    )
    .map(|read| read.bundle)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynchronizedCreateObservationPacketBundle {
    pub bundle: CreateObservationPacketBundle,
    pub discarded_bytes: u16,
}

pub fn read_synchronized_observation_bundle<P: CreateUartProvider>(
    provider: &mut P,
    deadline_tick: u64,
    maximum_discarded_bytes: u16,
) -> Result<SynchronizedCreateObservationPacketBundle, CreateOiFailure> {
    let mut frame = [0_u8; CREATE_OBSERVATION_FRAME_BYTES];
    let read = read_expected_stream_frame(
        provider,
        &[
            CREATE_GROUP_ZERO_PACKET_ID,
            CREATE_CHARGING_SOURCES_PACKET_ID,
        ],
        deadline_tick,
        maximum_discarded_bytes,
        &mut frame,
    )?;
    Ok(SynchronizedCreateObservationPacketBundle {
        bundle: decode_observation_bundle(&frame)?,
        discarded_bytes: read.discarded_bytes,
    })
}

pub fn decode_observation_bundle(
    frame: &[u8],
) -> Result<CreateObservationPacketBundle, CreateOiFailure> {
    if frame.len() < CREATE_OBSERVATION_FRAME_BYTES {
        return Err(CreateOiFailure::TruncatedFrame);
    }
    if frame.len() != CREATE_OBSERVATION_FRAME_BYTES
        || frame[0] != STREAM_HEADER
        || usize::from(frame[1]) != BUNDLE_PAYLOAD_BYTES
        || frame[2] != CREATE_GROUP_ZERO_PACKET_ID
        || frame[2 + 1 + GROUP_ZERO_BYTES] != CREATE_CHARGING_SOURCES_PACKET_ID
        || frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) != 0
    {
        return Err(CreateOiFailure::MalformedFrame);
    }
    let group_start = 3;
    let group_end = group_start + GROUP_ZERO_BYTES;
    Ok(CreateObservationPacketBundle {
        group_zero: decode_sensor_packet(
            CREATE_GROUP_ZERO_PACKET_ID,
            &frame[group_start..group_end],
        )?,
        charging_sources: decode_sensor_packet(
            CREATE_CHARGING_SOURCES_PACKET_ID,
            &frame[group_end + 1..group_end + 2],
        )?,
    })
}

const _: () = assert!(CREATE_OI_MAX_COMMAND_BYTES >= 4);

#[cfg(test)]
mod tests;
