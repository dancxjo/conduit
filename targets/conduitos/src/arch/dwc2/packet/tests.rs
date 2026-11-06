use super::super::test_io::Io;
use super::*;
use std::vec::Vec;
fn controller() -> Controller<Io> {
    Controller {
        io: Io::new(),
        prepared: true,
    }
}
fn packet(input: bool) -> RootPacket {
    RootPacket {
        address: 1,
        maximum_packet: 8,
        input,
        pid: PacketPid::Data1,
    }
}
#[test]
fn output_writes_only_actual_bytes_with_zero_fifo_padding() {
    let mut c = controller();
    assert_eq!(c.root_packet(packet(false), &mut [1, 2, 3, 4, 5]), Ok(5));
    let words: Vec<_> =
        c.io.writes
            .iter()
            .filter(|(offset, _)| *offset == FIFO)
            .map(|(_, word)| *word)
            .collect();
    assert_eq!(words, [0x04030201, 5]);
    assert!(c.is_prepared());
}
#[test]
fn input_drains_actual_octets_before_acknowledged_completion() {
    let mut c = controller();
    c.io.rx.push_back((2 << 17) | (5 << 4));
    c.io.data.extend([0x04030201, 0xeeeeee05]);
    let mut bytes = [0xcc; 8];
    assert_eq!(c.root_packet(packet(true), &mut bytes), Ok(5));
    assert_eq!(bytes, [1, 2, 3, 4, 5, 0xcc, 0xcc, 0xcc]);
    assert!(c.io.data.is_empty());
}
#[test]
fn zero_length_input_packet_remains_a_real_observation() {
    let mut c = controller();
    c.io.rx.push_back(2 << 17);
    assert_eq!(c.root_packet(packet(true), &mut [0; 8]), Ok(0));
    let mut c = controller();
    assert_eq!(
        c.root_packet(packet(true), &mut [0; 8]),
        Err(PacketError::ReceiveEnvelope)
    );
}
#[test]
fn dispositions_do_not_retry_or_advance_pid() {
    for (bit, expected) in [
        (3, PacketError::Stall),
        (4, PacketError::Nak),
        (6, PacketError::Nyet),
        (7, PacketError::Transaction),
        (10, PacketError::Transaction),
    ] {
        let mut c = controller();
        c.io.finish = HALTED | (1 << bit);
        assert_eq!(c.root_packet(packet(false), &mut []), Err(expected));
        assert_eq!(
            c.io.writes
                .iter()
                .filter(|(offset, value)| *offset == CHARACTER && value & HALT == 0)
                .count(),
            1
        );
    }
}
#[test]
fn malformed_receive_is_drained_and_never_exposed_as_input() {
    let mut c = controller();
    c.io.rx.push_back((2 << 17) | (12 << 4));
    c.io.data.extend([1, 2, 3]);
    let mut bytes = [0xcc; 8];
    assert_eq!(
        c.root_packet(packet(true), &mut bytes),
        Err(PacketError::ReceiveEnvelope)
    );
    assert!(c.io.data.is_empty());
    assert_eq!(bytes, [0xcc; 8]);
}
#[test]
fn deadline_requires_halt_acknowledgement_or_fences_controller() {
    for stuck in [false, true] {
        let mut c = controller();
        c.io.finish = 0;
        c.io.stuck_halt = stuck;
        assert_eq!(
            c.root_packet(packet(false), &mut []),
            Err(if stuck {
                PacketError::Retirement
            } else {
                PacketError::Deadline
            })
        );
        assert_eq!(c.is_prepared(), !stuck);
        if stuck {
            assert_eq!(
                c.root_packet(packet(false), &mut []),
                Err(PacketError::NotPrepared)
            );
        }
    }
}
#[test]
fn unsupported_route_and_geometry_never_submit() {
    let mut c = controller();
    c.io.port |= 1 << 17;
    assert_eq!(
        c.root_packet(packet(false), &mut []),
        Err(PacketError::UnsupportedSpeed)
    );
    assert!(c.io.writes.is_empty());
    let mut c = controller();
    let mut p = packet(false);
    p.address = 128;
    assert_eq!(c.root_packet(p, &mut []), Err(PacketError::Geometry));
    assert!(c.io.writes.is_empty());
}
#[test]
fn data_and_setup_pid_register_values_are_exact() {
    for (pid, bits) in [
        (PacketPid::Data0, 0),
        (PacketPid::Data1, 2),
        (PacketPid::Setup, 3),
    ] {
        let mut c = controller();
        let mut p = packet(false);
        p.pid = pid;
        assert_eq!(c.root_packet(p, &mut [0; 8]), Ok(8));
        assert!(c.io.writes.contains(&(SIZE, (bits << 29) | (1 << 19) | 8)));
    }
}

#[test]
fn first_nak_requires_explicit_halt_without_waiting_for_deadline() {
    let mut c = controller();
    c.io.finish = 1 << 4;
    assert_eq!(c.root_packet(packet(false), &mut []), Err(PacketError::Nak));
    assert!(
        c.io.writes
            .iter()
            .any(|(offset, word)| *offset == CHARACTER && word & HALT != 0)
    );
    assert!(c.is_prepared());
}
#[test]
fn transfer_complete_does_not_hide_untransmitted_bytes() {
    let mut c = controller();
    c.io.residue = 1;
    assert_eq!(
        c.root_packet(packet(false), &mut [7; 4]),
        Err(PacketError::Transaction)
    );
}

#[test]
fn overcurrent_removes_power_and_fences_further_packets() {
    let mut c = controller();
    c.io.port |= OVERCURRENT;
    assert_eq!(
        c.root_packet(packet(false), &mut []),
        Err(PacketError::Overcurrent)
    );
    assert!(!c.is_prepared());
    assert!(
        c.io.writes
            .iter()
            .any(|(offset, value)| *offset == PORT && value & PORT_POWER == 0)
    );
    assert_eq!(
        c.root_packet(packet(false), &mut []),
        Err(PacketError::NotPrepared)
    );
}
