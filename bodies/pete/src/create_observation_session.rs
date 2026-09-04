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
    last_stream_discarded_bytes: Option<u16>,
}

impl CreateObservationSession {
    pub const fn new() -> Self {
        Self {
            state: CreateObservationSessionState::New,
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
                write_command(
                    provider,
                    &encode_mode(CreateOiModeRequest::Safe).expect("Safe has one command"),
                )
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
mod tests {
    use super::*;
    use crate::UartProfile;
    use std::collections::VecDeque;

    struct Provider {
        available: bool,
        fail_write_at: Option<usize>,
        writes: Vec<Vec<u8>>,
        read: VecDeque<u8>,
    }

    impl CreateUartProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            self.available
        }
        fn profile(&self) -> UartProfile {
            UartProfile::CREATE_OI
        }
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            if self.fail_write_at == Some(self.writes.len()) {
                return Err(());
            }
            self.writes.push(bytes.to_vec());
            Ok(())
        }
        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
            Ok(self.read.pop_front())
        }
    }

    fn provider(frame: &[u8]) -> Provider {
        Provider {
            available: true,
            fail_write_at: None,
            writes: vec![],
            read: frame.iter().copied().collect(),
        }
    }

    fn group_zero() -> [u8; GROUP_ZERO_BYTES] {
        let mut bytes = [0_u8; GROUP_ZERO_BYTES];
        bytes[16] = 3;
        bytes[17..19].copy_from_slice(&14_000_u16.to_be_bytes());
        bytes[19..21].copy_from_slice(&100_i16.to_be_bytes());
        bytes[22..24].copy_from_slice(&1_000_u16.to_be_bytes());
        bytes[24..26].copy_from_slice(&2_000_u16.to_be_bytes());
        bytes
    }

    fn frame() -> Vec<u8> {
        let mut frame = vec![STREAM_HEADER, BUNDLE_PAYLOAD_BYTES as u8, 0];
        frame.extend_from_slice(&group_zero());
        frame.extend_from_slice(&[34, 2]);
        let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
        frame.push(0_u8.wrapping_sub(sum));
        frame
    }

    #[test]
    fn exact_session_starts_reads_correlated_observation_and_pauses() {
        let mut provider = provider(&frame());
        let mut session = CreateObservationSession::new();
        session.start(&mut provider).unwrap();
        assert_eq!(provider.writes, [vec![128], vec![131], vec![148, 2, 0, 34]]);
        let observation = session.read(&mut provider, 10).unwrap();
        assert_eq!(session.last_stream_discarded_bytes(), Some(0));
        assert_eq!(observation.charging_sources.bits(), 2);
        assert_eq!(
            observation
                .group_zero
                .charging
                .battery()
                .unwrap()
                .unwrap()
                .charge_permille(),
            500
        );
        session.pause(&mut provider).unwrap();
        assert_eq!(provider.writes.last().unwrap(), &[150, 0]);
        assert_eq!(session.state(), CreateObservationSessionState::Paused);
        assert_eq!(
            session.read(&mut provider, 10),
            Err(CreateObservationFailure::InvalidState)
        );
    }

    #[test]
    fn session_uses_bounded_create_stream_synchronization() {
        let mut bytes = vec![0xaa, 0xbb, 0xcc];
        bytes.extend_from_slice(&frame());
        let mut provider = provider(&bytes);
        let mut session = CreateObservationSession::new();
        session.start(&mut provider).unwrap();
        let observation = session.read(&mut provider, 10).unwrap();
        assert_eq!(observation.charging_sources.bits(), 2);
        assert_eq!(session.last_stream_discarded_bytes(), Some(3));
    }

    #[test]
    fn truncation_order_checksum_and_no_response_remain_distinct() {
        let valid = frame();
        assert_eq!(
            decode_observation_bundle(&valid[..valid.len() - 1]),
            Err(CreateOiFailure::TruncatedFrame)
        );
        let mut wrong_order = valid.clone();
        wrong_order[2] = 34;
        assert_eq!(
            decode_observation_bundle(&wrong_order),
            Err(CreateOiFailure::MalformedFrame)
        );
        let mut corrupt = valid;
        corrupt[3] ^= 1;
        assert_eq!(
            decode_observation_bundle(&corrupt),
            Err(CreateOiFailure::MalformedFrame)
        );
        let mut silent = provider(&[]);
        assert_eq!(
            read_observation_bundle(&mut silent, 10),
            Err(CreateOiFailure::DeviceNoResponse)
        );
    }

    #[test]
    fn provider_failure_has_no_retry_and_read_failure_can_still_pause() {
        let mut write_failure = provider(&frame());
        write_failure.fail_write_at = Some(1);
        let mut failed = CreateObservationSession::new();
        assert_eq!(
            failed.start(&mut write_failure),
            Err(CreateObservationFailure::Protocol(
                CreateOiFailure::WriteFailed
            ))
        );
        assert_eq!(write_failure.writes, [vec![128]]);
        assert_eq!(failed.state(), CreateObservationSessionState::StartFailed);
        assert_eq!(
            failed.pause(&mut write_failure),
            Err(CreateObservationFailure::InvalidState)
        );

        let mut truncated = provider(&frame()[..5]);
        let mut session = CreateObservationSession::new();
        session.start(&mut truncated).unwrap();
        assert_eq!(
            session.read(&mut truncated, 10),
            Err(CreateObservationFailure::Protocol(
                CreateOiFailure::TruncatedFrame
            ))
        );
        assert_eq!(session.state(), CreateObservationSessionState::ReadFailed);
        session.pause(&mut truncated).unwrap();
        assert_eq!(session.state(), CreateObservationSessionState::Paused);
    }
}
