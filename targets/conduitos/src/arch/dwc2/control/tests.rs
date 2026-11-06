use super::super::{packet::*, test_io::Io};
use super::*;
use std::vec::Vec;
fn controller() -> Controller<Io> {
    Controller {
        io: Io::new(),
        prepared: true,
    }
}
fn completion(bytes: Option<&[u8]>) -> (u32, Vec<u32>, Vec<u32>) {
    let Some(bytes) = bytes else {
        return (HALTED | 1, Vec::new(), Vec::new());
    };
    let words = bytes
        .chunks(4)
        .map(|chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            u32::from_le_bytes(word)
        })
        .collect();
    (
        HALTED | 1,
        vec![(2 << 17) | ((bytes.len() as u32) << 4)],
        words,
    )
}
fn sizes(c: &Controller<Io>) -> Vec<u32> {
    c.io.writes
        .iter()
        .filter(|(offset, _)| *offset == 0x510)
        .map(|(_, word)| *word)
        .collect()
}
#[test]
fn in_control_preserves_short_actual_data_and_uses_opposite_status_direction() {
    let mut c = controller();
    c.io.script.extend([
        completion(None),
        completion(Some(&[1, 2, 3])),
        completion(None),
    ]);
    let mut input = [0xcc; 18];
    assert_eq!(
        c.root_control(0, 8, [0x80, 6, 0, 1, 0, 0, 18, 0], &[], &mut input),
        Ok(3)
    );
    assert_eq!(&input[..4], &[1, 2, 3, 0xcc]);
    assert_eq!(
        sizes(&c),
        [
            (3 << 29) | (1 << 19) | 8,
            (2 << 29) | (1 << 19) | 8,
            (2 << 29) | (1 << 19)
        ]
    );
}
#[test]
fn multipacket_out_toggles_data_and_expects_zero_length_in_status() {
    let mut c = controller();
    c.io.script.extend([
        completion(None),
        completion(None),
        completion(None),
        completion(Some(&[])),
    ]);
    assert_eq!(
        c.root_control(1, 8, [0x21, 9, 0, 0, 0, 0, 9, 0], &[7; 9], &mut []),
        Ok(9)
    );
    assert_eq!(
        sizes(&c),
        [
            (3 << 29) | (1 << 19) | 8,
            (2 << 29) | (1 << 19) | 8,
            (1 << 19) | 1,
            (2 << 29) | (1 << 19) | 8
        ]
    );
}
#[test]
fn no_data_ignores_direction_for_status_and_does_not_interpret_opcode() {
    for request_type in [0x23, 0xa3] {
        let mut c = controller();
        c.io.script
            .extend([completion(None), completion(Some(&[]))]);
        assert_eq!(
            c.root_control(1, 8, [request_type, 0xff, 8, 0, 2, 0, 0, 0], &[], &mut []),
            Ok(0)
        );
        assert_eq!(sizes(&c).len(), 2);
    }
}
#[test]
fn invalid_envelope_has_no_hardware_effect_and_nak_is_not_retried() {
    let mut c = controller();
    assert_eq!(
        c.root_control(1, 8, [0x80, 6, 0, 1, 0, 0, 18, 0], &[], &mut [0; 17]),
        Err(PacketError::Geometry)
    );
    assert!(c.io.writes.is_empty());
    c.io.script
        .push_back((HALTED | (1 << 4), Vec::new(), Vec::new()));
    assert_eq!(
        c.root_control(1, 8, [0x23, 3, 8, 0, 2, 0, 0, 0], &[], &mut []),
        Err(PacketError::Nak)
    );
    assert_eq!(sizes(&c).len(), 1);
}
#[test]
fn status_payload_and_data_overrun_are_refused() {
    let mut c = controller();
    c.io.script
        .extend([completion(None), completion(Some(&[1]))]);
    assert_eq!(
        c.root_control(1, 8, [0x23, 3, 8, 0, 2, 0, 0, 0], &[], &mut []),
        Err(PacketError::ReceiveEnvelope)
    );
    let mut c = controller();
    c.io.script
        .extend([completion(None), completion(Some(&[1; 8]))]);
    assert_eq!(
        c.root_control(1, 8, [0x80, 6, 0, 1, 0, 0, 7, 0], &[], &mut [0xcc; 7]),
        Err(PacketError::ReceiveEnvelope)
    );
}
#[test]
fn maximum_envelope_uses_exactly_32_data_packets_and_one_status() {
    let mut c = controller();
    c.io.script.push_back(completion(None));
    for _ in 0..32 {
        c.io.script.push_back(completion(Some(&[0x5a; 8])));
    }
    c.io.script.push_back(completion(None));
    let mut bytes = [0; 256];
    assert_eq!(
        c.root_control(1, 8, [0x80, 0xff, 0, 0, 0, 0, 0, 1], &[], &mut bytes),
        Ok(256)
    );
    assert_eq!(bytes, [0x5a; 256]);
    assert_eq!(sizes(&c).len(), 34);
    assert!(c.io.script.is_empty());
}
