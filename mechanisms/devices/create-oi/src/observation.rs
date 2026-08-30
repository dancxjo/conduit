//! Bounded Create 1 sensor requests below portable robotics meaning.

use crate::{
    encode_query_sensor, read_query_sensor_packet, write_command, CreateOiFailure,
    CreateUartProvider,
};

pub const CREATE_1_GROUP_ZERO_PACKET_ID: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Create1GroupZero {
    pub left_bumper_pressed: bool,
    pub right_bumper_pressed: bool,
    pub charging_state: u8,
    pub millivolts: u16,
    pub milliamps: i16,
    pub temperature_celsius: i8,
    pub charge_mah: u16,
    pub capacity_mah: u16,
}

/// Perform exactly one request/response transaction for Create 1 packet 0.
///
/// The caller owns mode admission and the finite deadline. This function does
/// not retry, start a stream, or reinterpret the device data as portable
/// robotics meaning.
pub fn query_create1_group_zero<P: CreateUartProvider>(
    provider: &mut P,
    deadline_tick: u64,
) -> Result<Create1GroupZero, CreateOiFailure> {
    let query = encode_query_sensor(CREATE_1_GROUP_ZERO_PACKET_ID)?;
    write_command(provider, &query)?;
    let packet = read_query_sensor_packet(provider, CREATE_1_GROUP_ZERO_PACKET_ID, deadline_tick)?;
    let bytes = packet.bytes();
    Ok(Create1GroupZero {
        left_bumper_pressed: bytes[0] & (1 << 1) != 0,
        right_bumper_pressed: bytes[0] & 1 != 0,
        charging_state: bytes[16],
        millivolts: u16::from_be_bytes([bytes[17], bytes[18]]),
        milliamps: i16::from_be_bytes([bytes[19], bytes[20]]),
        temperature_celsius: bytes[21] as i8,
        charge_mah: u16::from_be_bytes([bytes[22], bytes[23]]),
        capacity_mah: u16::from_be_bytes([bytes[24], bytes[25]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UartProfile;
    use std::{collections::VecDeque, vec, vec::Vec};

    struct Provider {
        writes: Vec<Vec<u8>>,
        read: VecDeque<u8>,
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
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
            Ok(self.read.pop_front())
        }
    }

    #[test]
    fn group_zero_query_is_single_shot_and_preserves_create_truth() {
        let mut payload = [0_u8; 26];
        payload[0] = 0b0000_0011;
        payload[16] = 2;
        payload[17..19].copy_from_slice(&14_400_u16.to_be_bytes());
        payload[19..21].copy_from_slice(&(-320_i16).to_be_bytes());
        payload[21] = 29;
        payload[22..24].copy_from_slice(&1_200_u16.to_be_bytes());
        payload[24..26].copy_from_slice(&2_400_u16.to_be_bytes());
        let mut provider = Provider {
            writes: Vec::new(),
            read: VecDeque::from(payload),
        };

        let observed = query_create1_group_zero(&mut provider, 10).unwrap();
        assert_eq!(provider.writes, [vec![142, 0]]);
        assert!(observed.left_bumper_pressed);
        assert!(observed.right_bumper_pressed);
        assert_eq!(observed.charging_state, 2);
        assert_eq!(observed.millivolts, 14_400);
        assert_eq!(observed.milliamps, -320);
        assert_eq!(observed.temperature_celsius, 29);
        assert_eq!(observed.charge_mah, 1_200);
        assert_eq!(observed.capacity_mah, 2_400);
    }
}
