use super::*;
use std::collections::VecDeque;
use std::vec;
use std::vec::Vec;

struct Provider {
    available: bool,
    profile: UartProfile,
    written: Vec<u8>,
    read: VecDeque<u8>,
}

impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        self.available
    }

    fn profile(&self) -> UartProfile {
        self.profile
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.written.extend_from_slice(bytes);
        Ok(())
    }

    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, ()> {
        Ok(self.read.pop_front())
    }
}

fn provider() -> Provider {
    Provider {
        available: true,
        profile: UartProfile::CREATE_OI,
        written: vec![],
        read: VecDeque::new(),
    }
}

fn frame(packet_id: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![STREAM_HEADER, payload.len() as u8 + 1, packet_id];
    frame.extend_from_slice(payload);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    frame
}

#[test]
fn exact_profile_and_wheel_order_are_enforced() {
    let mut provider = provider();
    write_command(&mut provider, &encode_drive_direct(-100, 250).unwrap()).unwrap();
    assert_eq!(provider.written, [145, 0, 250, 255, 156]);
    provider.profile = UartProfile::CREATE_OI_19200;
    write_command(&mut provider, &encode_stop()).unwrap();
    provider.profile.baud = 115_200;
    assert!(matches!(
        write_command(&mut provider, &encode_stop()),
        Err(CreateOiFailure::WrongUartProfile { .. })
    ));
}

#[test]
fn create_1_v2_profile_and_led_bits_are_exact() {
    assert_eq!(CREATE_1_OI_PROTOCOL_VERSION, 2);
    assert_eq!(CREATE_OI_BAUD, 57_600);
    assert_eq!(CREATE_OI_ALTERNATE_BAUD, 19_200);
    assert!(UartProfile::CREATE_OI.is_create_oi());
    assert!(UartProfile::CREATE_OI_19200.is_create_oi());
    assert_eq!(CREATE_1_PLAY_LED_MASK, 0x02);
    assert_eq!(CREATE_1_ADVANCE_LED_MASK, 0x08);
    assert_eq!(CREATE_1_LED_MASK, 0x0a);
    assert_eq!(
        encode_lights(0xff, 128, 255).as_bytes(),
        &[139, 0x0a, 128, 255]
    );
}

#[test]
fn frames_are_finite_checked_and_not_promoted_when_corrupt() {
    let valid = frame(7, &[0b0000_0011]);
    assert_eq!(decode_stream_frame(7, &valid).unwrap().bytes(), &[3]);
    assert_eq!(
        decode_stream_frame(7, &valid[..valid.len() - 1]),
        Err(CreateOiFailure::TruncatedFrame)
    );
    let mut corrupt = valid;
    corrupt[3] = 0xff;
    assert_eq!(
        decode_stream_frame(7, &corrupt),
        Err(CreateOiFailure::MalformedFrame)
    );
    assert_eq!(
        encode_sensor_stream(33),
        Err(CreateOiFailure::UnsupportedPacket(33))
    );
    for reserved_button_bit in [0x02, 0x08] {
        assert_eq!(
            decode_sensor_packet(18, &[reserved_button_bit]),
            Err(CreateOiFailure::MalformedFrame)
        );
    }
    for valid_buttons in [0x00, 0x01, 0x04, 0x05] {
        assert_eq!(
            decode_sensor_packet(18, &[valid_buttons]).unwrap().bytes(),
            [valid_buttons]
        );
    }
}

#[test]
fn embedded_supervision_stream_is_exact_and_finite() {
    assert_eq!(
        encode_sensor_stream_triplet(0, 34, 35).unwrap().as_bytes(),
        &[148, 3, 0, 34, 35]
    );
    assert_eq!(
        encode_sensor_stream_triplet(0, 34, 33),
        Err(CreateOiFailure::UnsupportedPacket(33))
    );
}

#[test]
fn absent_provider_and_no_bytes_remain_distinct() {
    let mut absent = provider();
    absent.available = false;
    assert_eq!(
        write_command(&mut absent, &encode_start()),
        Err(CreateOiFailure::ProviderUnavailable)
    );
    let mut silent = provider();
    assert_eq!(
        read_stream_packet(&mut silent, 35, 10),
        Err(CreateOiFailure::DeviceNoResponse)
    );
    let mut partial = provider();
    partial.read.push_back(STREAM_HEADER);
    assert_eq!(
        read_stream_packet(&mut partial, 35, 10),
        Err(CreateOiFailure::TruncatedFrame)
    );
}

#[test]
fn single_packet_query_is_allow_listed_bounded_and_reads_raw_payload() {
    let mut queried = provider();
    write_command(&mut queried, &encode_query_sensor(35).unwrap()).unwrap();
    assert_eq!(queried.written, [142, 35]);
    queried.read.push_back(2);
    assert_eq!(
        read_query_sensor_packet(&mut queried, 35, 10)
            .unwrap()
            .bytes(),
        [2]
    );
    assert_eq!(
        encode_query_sensor(33),
        Err(CreateOiFailure::UnsupportedPacket(33))
    );
    let mut malformed = provider();
    malformed.read.push_back(4);
    assert_eq!(
        read_query_sensor_packet(&mut malformed, 35, 10),
        Err(CreateOiFailure::MalformedFrame)
    );
}
