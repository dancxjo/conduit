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
