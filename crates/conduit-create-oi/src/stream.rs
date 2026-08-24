//! Bounded synchronization for Create OI stream frames.

use crate::{
    require_provider, sensor_packet_len, CreateOiFailure, CreateUartProvider, STREAM_HEADER,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOiStreamRead {
    pub frame_bytes: u16,
    pub discarded_bytes: u16,
}

/// Read one exact requested Create OI stream frame into caller-owned storage.
///
/// Bytes before a matching header, length, packet sequence, and checksum are
/// discarded only within `maximum_discarded_bytes`. This is synchronization of
/// one already-requested stream, not retransmission or retry policy.
pub fn read_expected_stream_frame<P: CreateUartProvider>(
    provider: &mut P,
    packet_ids: &[u8],
    deadline_tick: u64,
    maximum_discarded_bytes: u16,
    frame: &mut [u8],
) -> Result<CreateOiStreamRead, CreateOiFailure> {
    require_provider(provider)?;
    let payload_bytes = expected_payload_bytes(packet_ids)?;
    let frame_bytes = payload_bytes
        .checked_add(3)
        .ok_or(CreateOiFailure::MalformedFrame)?;
    if packet_ids.is_empty() || payload_bytes > usize::from(u8::MAX) || frame.len() != frame_bytes {
        return Err(CreateOiFailure::MalformedFrame);
    }

    let mut received = 0_usize;
    let mut discarded = 0_u16;
    let mut observed_any = false;
    let mut rejected_candidate = false;
    loop {
        let next = provider
            .read_byte(deadline_tick)
            .map_err(|_| CreateOiFailure::ReadFailed)?;
        let Some(byte) = next else {
            return Err(if !observed_any {
                CreateOiFailure::DeviceNoResponse
            } else if received == 0 && rejected_candidate {
                CreateOiFailure::MalformedFrame
            } else {
                CreateOiFailure::TruncatedFrame
            });
        };
        observed_any = true;
        frame[received] = byte;
        received += 1;

        while received != 0 && !prefix_matches(packet_ids, payload_bytes, &frame[..received]) {
            rejected_candidate |= frame[0] == STREAM_HEADER;
            let retained = frame[1..received]
                .iter()
                .position(|candidate| *candidate == STREAM_HEADER)
                .map_or(0, |position| received - position - 1);
            let newly_discarded = received - retained;
            discarded = add_discarded(discarded, newly_discarded, maximum_discarded_bytes)?;
            if retained != 0 {
                frame.copy_within(received - retained..received, 0);
            }
            received = retained;
        }

        if received == frame_bytes {
            if frame
                .iter()
                .fold(0_u8, |sum, value| sum.wrapping_add(*value))
                == 0
            {
                return Ok(CreateOiStreamRead {
                    frame_bytes: u16::try_from(frame_bytes)
                        .map_err(|_| CreateOiFailure::MalformedFrame)?,
                    discarded_bytes: discarded,
                });
            }
            rejected_candidate = true;
            let retained = frame[1..received]
                .iter()
                .position(|candidate| *candidate == STREAM_HEADER)
                .map_or(0, |position| received - position - 1);
            discarded = add_discarded(discarded, received - retained, maximum_discarded_bytes)?;
            if retained != 0 {
                frame.copy_within(received - retained..received, 0);
            }
            received = retained;
        }
    }
}

fn expected_payload_bytes(packet_ids: &[u8]) -> Result<usize, CreateOiFailure> {
    packet_ids.iter().try_fold(0_usize, |total, packet_id| {
        let packet_bytes =
            sensor_packet_len(*packet_id).ok_or(CreateOiFailure::UnsupportedPacket(*packet_id))?;
        total
            .checked_add(packet_bytes + 1)
            .ok_or(CreateOiFailure::MalformedFrame)
    })
}

fn prefix_matches(packet_ids: &[u8], payload_bytes: usize, frame: &[u8]) -> bool {
    if frame.first().copied() != Some(STREAM_HEADER) {
        return false;
    }
    if frame.len() >= 2 && usize::from(frame[1]) != payload_bytes {
        return false;
    }
    let mut packet_offset = 2_usize;
    for packet_id in packet_ids {
        if frame.len() <= packet_offset {
            return true;
        }
        if frame[packet_offset] != *packet_id {
            return false;
        }
        let Some(packet_bytes) = sensor_packet_len(*packet_id) else {
            return false;
        };
        packet_offset += packet_bytes + 1;
    }
    true
}

fn add_discarded(
    discarded: u16,
    newly_discarded: usize,
    maximum_discarded_bytes: u16,
) -> Result<u16, CreateOiFailure> {
    let total = usize::from(discarded).checked_add(newly_discarded).ok_or(
        CreateOiFailure::SynchronizationLimit {
            maximum_discarded_bytes,
        },
    )?;
    if total > usize::from(maximum_discarded_bytes) {
        return Err(CreateOiFailure::SynchronizationLimit {
            maximum_discarded_bytes,
        });
    }
    u16::try_from(total).map_err(|_| CreateOiFailure::SynchronizationLimit {
        maximum_discarded_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UartProfile;
    use std::collections::VecDeque;
    use std::vec;
    use std::vec::Vec;

    struct Provider {
        bytes: VecDeque<u8>,
        fail_at_read: Option<usize>,
        reads: usize,
    }

    impl CreateUartProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            true
        }

        fn profile(&self) -> UartProfile {
            UartProfile::CREATE_OI
        }

        fn write_all(&mut self, _: &[u8]) -> Result<(), Self::Error> {
            Ok(())
        }

        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
            if self.fail_at_read == Some(self.reads) {
                return Err(());
            }
            self.reads += 1;
            Ok(self.bytes.pop_front())
        }
    }

    fn provider(bytes: &[u8]) -> Provider {
        Provider {
            bytes: bytes.iter().copied().collect(),
            fail_at_read: None,
            reads: 0,
        }
    }

    fn frame(packet_id: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![STREAM_HEADER, payload.len() as u8 + 1, packet_id];
        bytes.extend_from_slice(payload);
        let sum = bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
        bytes.push(0_u8.wrapping_sub(sum));
        bytes
    }

    #[test]
    fn aligned_and_leading_noise_reads_are_distinct() {
        let valid = frame(35, &[2]);
        let mut output = [0_u8; 5];
        assert_eq!(
            read_expected_stream_frame(&mut provider(&valid), &[35], 10, 0, &mut output),
            Ok(CreateOiStreamRead {
                frame_bytes: 5,
                discarded_bytes: 0,
            })
        );
        assert_eq!(output, valid.as_slice());

        let mut noisy = vec![0xaa, 0xbb];
        noisy.extend_from_slice(&valid);
        assert_eq!(
            read_expected_stream_frame(&mut provider(&noisy), &[35], 10, 2, &mut output),
            Ok(CreateOiStreamRead {
                frame_bytes: 5,
                discarded_bytes: 2,
            })
        );
        assert_eq!(output, valid.as_slice());
    }

    #[test]
    fn false_prefixes_and_corrupt_frame_resynchronize_to_valid_frame() {
        let valid = frame(35, &[2]);
        let mut bytes = vec![STREAM_HEADER, 9, STREAM_HEADER, 2, 34];
        let mut corrupt = valid.clone();
        corrupt[3] = 3;
        bytes.extend_from_slice(&corrupt);
        bytes.extend_from_slice(&valid);
        let mut output = [0_u8; 5];
        let read =
            read_expected_stream_frame(&mut provider(&bytes), &[35], 10, 10, &mut output).unwrap();
        assert_eq!(read.discarded_bytes, 10);
        assert_eq!(output, valid.as_slice());
    }

    #[test]
    fn discard_limit_is_exact_and_machine_readable() {
        let valid = frame(35, &[2]);
        let mut bytes = vec![0xaa, 0xbb];
        bytes.extend_from_slice(&valid);
        let mut output = [0_u8; 5];
        assert_eq!(
            read_expected_stream_frame(&mut provider(&bytes), &[35], 10, 1, &mut output),
            Err(CreateOiFailure::SynchronizationLimit {
                maximum_discarded_bytes: 1,
            })
        );
        assert!(
            read_expected_stream_frame(&mut provider(&bytes), &[35], 10, 2, &mut output).is_ok()
        );
    }

    #[test]
    fn silence_partial_provider_failure_and_unsupported_packet_remain_distinct() {
        let mut output = [0_u8; 5];
        assert_eq!(
            read_expected_stream_frame(&mut provider(&[]), &[35], 10, 2, &mut output),
            Err(CreateOiFailure::DeviceNoResponse)
        );
        assert_eq!(
            read_expected_stream_frame(
                &mut provider(&[STREAM_HEADER, 2]),
                &[35],
                10,
                2,
                &mut output
            ),
            Err(CreateOiFailure::TruncatedFrame)
        );
        let mut failing = provider(&[STREAM_HEADER]);
        failing.fail_at_read = Some(1);
        assert_eq!(
            read_expected_stream_frame(&mut failing, &[35], 10, 2, &mut output),
            Err(CreateOiFailure::ReadFailed)
        );
        assert_eq!(
            read_expected_stream_frame(&mut provider(&[]), &[33], 10, 2, &mut output),
            Err(CreateOiFailure::UnsupportedPacket(33))
        );
    }
}
