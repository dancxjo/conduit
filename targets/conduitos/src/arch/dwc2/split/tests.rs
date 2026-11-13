use super::super::{TransactionTranslator, TransferType, test_io};
use super::*;
use std::vec::Vec;
fn route(periodic: bool) -> EndpointRoute {
    EndpointRoute {
        address: 2,
        endpoint: if periodic { 1 } else { 0 },
        maximum_packet: 8,
        transfer_type: if periodic {
            TransferType::Interrupt
        } else {
            TransferType::Control
        },
        translator: Some(TransactionTranslator {
            hub_address: 1,
            port: 2,
            low_speed: false,
        }),
    }
}
fn controller() -> Controller<test_io::Io> {
    let mut c = test_io::prepared();
    c.io.clock_step = 10;
    c
}
fn stage(status: u32) -> (u32, Vec<u32>, Vec<u32>) {
    (status, vec![], vec![])
}
#[test]
fn split_interrupt_input_continues_same_tt_transaction_without_replaying_start() {
    let mut c = controller();
    c.io.frame = 65527;
    c.io.script.extend([
        stage(0x22),
        stage(0x42),
        (3, vec![(2 << 17) | (3 << 4)], vec![0x00030201]),
    ]);
    let mut bytes = [0xcc; 8];
    assert_eq!(
        c.packet_on(route(true), PacketPid::Data0, true, &mut bytes),
        Ok(3)
    );
    assert_eq!(bytes, [1, 2, 3, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc]);
    let splits: Vec<_> =
        c.io.writes
            .iter()
            .filter(|(offset, _)| *offset == 0x504)
            .map(|(_, word)| *word)
            .collect();
    let start = (1 << 31) | (3 << 14) | (1 << 7) | 2;
    assert_eq!(splits, [start, start | (1 << 16), start | (1 << 16)]);
    assert!(c.is_quiescent());
}
#[test]
fn split_setup_transmits_payload_only_once_and_completes_without_output_bytes() {
    let mut c = controller();
    c.io.script.extend([stage(0x22), stage(3)]);
    assert_eq!(
        c.packet_on(route(false), PacketPid::Setup, false, &mut [7; 8]),
        Ok(8)
    );
    assert_eq!(
        c.io.writes
            .iter()
            .filter(|(offset, _)| *offset == 0x1000)
            .count(),
        2
    );
    let sizes: Vec<_> =
        c.io.writes
            .iter()
            .filter(|(offset, _)| *offset == 0x510)
            .map(|(_, word)| *word & 0x7ffff)
            .collect();
    assert_eq!(sizes, [8, 0]);
}
#[test]
fn exhausted_complete_split_window_fences_original_tt_obligation() {
    let mut c = controller();
    c.io.script
        .extend([stage(0x22), stage(0x42), stage(0x42), stage(0x42)]);
    assert_eq!(
        c.packet_on(route(true), PacketPid::Data0, true, &mut [0; 8]),
        Err(PacketError::Retirement)
    );
    assert!(!c.is_quiescent());
    assert!(!c.is_prepared());
    assert_eq!(
        c.packet_on(route(true), PacketPid::Data0, true, &mut [0; 8]),
        Err(PacketError::NotPrepared)
    );
}
#[test]
fn missing_frame_clock_and_invalid_tt_route_never_start_a_transaction() {
    let mut c = controller();
    c.io.frame = 7;
    c.io.frame_step = 0;
    assert_eq!(
        c.packet_on(route(false), PacketPid::Data0, false, &mut []),
        Err(PacketError::Deadline)
    );
    assert!(c.io.writes.is_empty());
    assert!(c.is_quiescent());
    let mut c = controller();
    let mut r = route(false);
    r.translator.as_mut().unwrap().port = 0;
    assert_eq!(
        c.packet_on(r, PacketPid::Data0, false, &mut []),
        Err(PacketError::Geometry)
    );
    assert!(c.io.writes.is_empty());
}
#[test]
fn tt_nak_is_terminal_and_does_not_replay_device_packet() {
    let mut c = controller();
    c.io.script.extend([stage(0x22), stage(0x12)]);
    assert_eq!(
        c.packet_on(route(false), PacketPid::Data1, true, &mut [0; 8]),
        Err(PacketError::Nak)
    );
    assert!(c.is_quiescent());
    assert!(c.is_prepared());
    assert_eq!(
        c.io.writes
            .iter()
            .filter(|(offset, _)| *offset == 0x504)
            .count(),
        2
    );
}
