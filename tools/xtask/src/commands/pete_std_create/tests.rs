use super::*;
use conduit_create_oi::{decode_sensor_packet, decode_stream_frame, UartProfile};
use std::collections::VecDeque;

struct Provider {
    writes: Vec<Vec<u8>>,
    fail_writes: Vec<usize>,
    reads: VecDeque<Result<Option<u8>, ()>>,
    read_deadlines: Vec<u64>,
}

impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let index = self.writes.len();
        self.writes.push(bytes.to_vec());
        if self.fail_writes.contains(&index) {
            Err(())
        } else {
            Ok(())
        }
    }

    fn read_byte(&mut self, deadline: u64) -> Result<Option<u8>, Self::Error> {
        self.read_deadlines.push(deadline);
        self.reads.pop_front().unwrap_or(Ok(None))
    }
}

fn provider(bytes: &[u8]) -> Provider {
    Provider {
        writes: Vec::new(),
        fail_writes: Vec::new(),
        reads: bytes.iter().copied().map(|byte| Ok(Some(byte))).collect(),
        read_deadlines: Vec::new(),
    }
}

fn frame(packet: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![19, payload.len() as u8 + 1, packet];
    bytes.extend_from_slice(payload);
    let sum = bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    bytes.push(0_u8.wrapping_sub(sum));
    bytes
}

#[test]
fn status_requests_use_exact_frames_one_deadline_and_pause_before_returning() {
    for (packet, value) in [(35, 2), (36, 2), (37, 1)] {
        let mut provider = provider(&frame(packet, &[value]));
        assert_eq!(
            read_framed_status(&mut provider, packet, 123)
                .unwrap()
                .bytes(),
            [value]
        );
        assert_eq!(provider.writes, [vec![148, 1, packet], vec![150, 0]]);
        assert_eq!(provider.read_deadlines, [123; 5]);
        assert!(provider.reads.is_empty());
    }
}

#[test]
fn bare_bytes_wrong_identity_prefix_checksum_and_payload_refuse_without_scanning() {
    let mut bad_checksum = frame(35, &[2]);
    bad_checksum[4] ^= 1;
    let mut bad_prefix = frame(35, &[2]);
    bad_prefix[0] = 0;
    for (bytes, failure) in [
        (vec![2], CreateOiFailure::TruncatedFrame),
        (frame(36, &[2]), CreateOiFailure::MalformedFrame),
        (bad_prefix, CreateOiFailure::MalformedFrame),
        (bad_checksum, CreateOiFailure::MalformedFrame),
        (frame(35, &[4]), CreateOiFailure::MalformedFrame),
        (vec![19, 2, 35], CreateOiFailure::TruncatedFrame),
        (vec![], CreateOiFailure::DeviceNoResponse),
    ] {
        let mut provider = provider(&bytes);
        assert_eq!(
            read_framed_status(&mut provider, 35, 456),
            Err(CreateStatusFailure {
                primary: Some(failure),
                pause_failure: None
            })
        );
        assert!(provider.read_deadlines.len() <= 5);
        assert!(provider
            .read_deadlines
            .iter()
            .all(|deadline| *deadline == 456));
        assert_eq!(provider.writes, [vec![148, 1, 35], vec![150, 0]]);
    }

    let mut bytes = frame(36, &[2]);
    let fresh = frame(35, &[2]);
    bytes.extend_from_slice(&fresh);
    let mut provider = provider(&bytes);
    assert!(read_framed_status(&mut provider, 35, 456).is_err());
    assert_eq!(provider.read_deadlines.len(), 5);
    assert_eq!(
        provider.reads,
        fresh
            .into_iter()
            .map(|byte| Ok::<_, ()>(Some(byte)))
            .collect::<VecDeque<_>>()
    );
}

#[test]
fn valid_group_zero_with_nested_fake_safe_frame_cannot_confirm_mode() {
    let nested = frame(35, &[2]);
    let mut payload = [0_u8; 26];
    // Voltage/current/temperature are unrestricted protocol bytes here. This
    // is a checksum-valid outer group-0 frame, not merely arbitrary noise.
    payload[17..22].copy_from_slice(&nested);
    decode_sensor_packet(0, &payload).unwrap();
    let outer = frame(0, &payload);
    decode_stream_frame(0, &outer).unwrap();
    assert!(outer.windows(nested.len()).any(|window| window == nested));
    let mut provider = provider(&outer);
    let error = establish_safe(&mut provider, 500).unwrap_err();
    assert!(error.contains("MalformedFrame"));
    assert_eq!(provider.read_deadlines.len(), 5);
    assert_eq!(provider.reads.len(), outer.len() - 5);
    assert_eq!(
        provider.writes,
        [vec![128], vec![131], vec![148, 1, 35], vec![150, 0]]
    );
}

#[test]
fn primary_and_pause_failures_are_retained_and_neither_admits_status() {
    for (bytes, fail_writes, primary, pause_failure) in [
        (
            frame(35, &[2]),
            vec![1],
            None,
            Some(CreateOiFailure::WriteFailed),
        ),
        (
            frame(36, &[2]),
            vec![1],
            Some(CreateOiFailure::MalformedFrame),
            Some(CreateOiFailure::WriteFailed),
        ),
        (
            vec![],
            vec![],
            Some(CreateOiFailure::DeviceNoResponse),
            None,
        ),
        (vec![], vec![0], Some(CreateOiFailure::WriteFailed), None),
        (
            vec![],
            vec![0, 1],
            Some(CreateOiFailure::WriteFailed),
            Some(CreateOiFailure::WriteFailed),
        ),
    ] {
        let mut provider = provider(&bytes);
        provider.fail_writes = fail_writes;
        assert_eq!(
            read_framed_status(&mut provider, 35, 100),
            Err(CreateStatusFailure {
                primary,
                pause_failure
            })
        );
        assert_eq!(provider.writes, [vec![148, 1, 35], vec![150, 0]]);
    }
    let mut provider = provider(&[]);
    provider.reads.push_back(Err(()));
    provider.fail_writes = vec![1];
    assert_eq!(
        read_framed_status(&mut provider, 35, 100),
        Err(CreateStatusFailure {
            primary: Some(CreateOiFailure::ReadFailed),
            pause_failure: Some(CreateOiFailure::WriteFailed)
        })
    );
    assert_eq!(provider.read_deadlines, [100]);
}

#[test]
fn unsupported_status_packet_has_no_effects() {
    let mut provider = provider(&[]);
    assert_eq!(
        read_framed_status(&mut provider, 0, 100),
        Err(CreateStatusFailure {
            primary: Some(CreateOiFailure::UnsupportedPacket(0)),
            pause_failure: None
        })
    );
    assert!(provider.writes.is_empty());
    assert!(provider.read_deadlines.is_empty());
}

#[test]
fn full_acquisition_retries_only_its_existing_bounded_attempts() {
    let mut provider = provider(&[]);
    provider.reads.push_back(Ok(None));
    for value in [2, 3] {
        provider
            .reads
            .extend(frame(35, &[value]).into_iter().map(|byte| Ok(Some(byte))));
    }
    assert_eq!(establish_full(&mut provider, 1).unwrap(), OiMode::Full);
    assert_eq!(provider.writes.len(), 12);
    for attempt in provider.writes.as_chunks::<4>().0 {
        assert_eq!(
            attempt,
            &[vec![128], vec![132], vec![148, 1, 35], vec![150, 0]]
        );
    }

    let mut unavailable = self::provider(&[]);
    let error = establish_full(&mut unavailable, 1).unwrap_err();
    assert!(error.contains("after 10 attempts"));
    assert_eq!(unavailable.writes.len(), 40);
    assert_eq!(unavailable.read_deadlines.len(), 10);
}

#[test]
fn safe_acquisition_is_one_attempt_and_pause_failure_never_retries_full() {
    let mut provider = provider(&frame(35, &[3]));
    assert!(establish_safe(&mut provider, 1)
        .unwrap_err()
        .contains("after 1 attempts"));
    assert_eq!(
        provider.writes,
        [vec![128], vec![131], vec![148, 1, 35], vec![150, 0]]
    );

    let mut provider = self::provider(&[]);
    provider.fail_writes = vec![3];
    let error = establish_full(&mut provider, 1).unwrap_err();
    assert!(error.contains("DeviceNoResponse"));
    assert!(error.contains("pause_failure: Some(WriteFailed)"));
    assert_eq!(provider.writes.len(), 4);
}
