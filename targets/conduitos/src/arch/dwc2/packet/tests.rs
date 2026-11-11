use super::super::test_io::Io;
use super::*;
use std::vec::Vec;
fn controller() -> Controller<Io> {
    Controller {
        io: Io::new(),
        prepared: true,
        quiescent: true,
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
    assert_eq!(c.io.failures[0].phase, "rx-missing");
    assert_eq!(c.io.failures[0].receive_status, None);
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
    assert_eq!(c.io.failures[0].phase, "rx-envelope");
    assert_eq!(c.io.failures[0].receive_status, Some((2 << 17) | (12 << 4)));
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
        assert_eq!(c.is_quiescent(), !stuck);
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
fn completed_out_can_retain_programmed_transfer_size() {
    let mut c = controller();
    c.io.finish = 0x21; // ACK and transfer complete, as observed on the B+.
    c.io.residue = 0x20000008;
    assert_eq!(c.root_packet(packet(false), &mut [7; 8]), Ok(8));
    assert!(c.is_quiescent());
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

#[test]
fn pio_transfer_complete_is_followed_by_explicit_channel_halt() {
    let mut c = controller();
    c.io.finish = 1;
    assert_eq!(c.root_packet(packet(false), &mut [7; 4]), Ok(4));
    assert!(
        c.io.writes
            .iter()
            .any(|(offset, bits)| *offset == CHARACTER && bits & HALT != 0)
    );
    assert!(c.is_quiescent());
}

#[test]
fn halt_acknowledgement_requires_receive_status_consumption() {
    let mut c = controller();
    c.io.finish = 1; // Transfer complete, not yet halted in slave/PIO mode.
    assert_eq!(c.root_packet(packet(false), &mut [7; 4]), Ok(4));
    assert!(c.io.rx.is_empty());
    assert!(c.is_quiescent());
    assert!(c.io.failures.is_empty());
}

#[test]
fn stuck_halt_reports_original_outcome_and_keeps_channel_fenced() {
    let mut c = controller();
    c.io.finish = 1 << 4;
    c.io.stuck_halt = true;
    assert_eq!(
        c.root_packet(packet(false), &mut []),
        Err(PacketError::Retirement)
    );
    assert!(!c.is_quiescent());
    let failure = c.io.failures.last().unwrap();
    assert_eq!(failure.phase, "halt");
    assert_eq!(failure.outcome, Some(PacketError::Nak));
    assert_ne!(failure.character & ENABLE, 0);
    assert!(c.io.rx.is_empty());
}

#[test]
fn halt_waits_for_queue_room_without_resubmitting_data() {
    let mut c = controller();
    c.io.finish = 1;
    c.io.queue_slots.extend([1, 0, 0, 1]);
    assert_eq!(c.root_packet(packet(false), &mut [9; 4]), Ok(4));
    let writes: Vec<_> =
        c.io.writes
            .iter()
            .filter(|(r, _)| *r == CHARACTER)
            .collect();
    assert_eq!(writes.len(), 3); // One packet, queue flush, then one halt.
    assert_eq!(writes[1].1 & (HALT | ENABLE), HALT);
    assert_eq!(writes[2].1 & (HALT | ENABLE), HALT | ENABLE);
    assert!(c.io.rx.is_empty());
}

#[test]
fn late_receive_data_is_drained_but_never_promoted_to_success() {
    let mut c = controller();
    c.io.finish = 1;
    c.io.late_halt_data = true;
    let mut bytes = [9; 4];
    assert_eq!(
        c.root_packet(packet(false), &mut bytes),
        Err(PacketError::ReceiveEnvelope)
    );
    assert_eq!(bytes, [9; 4]);
    assert!(c.io.rx.is_empty() && c.io.data.is_empty());
    assert!(c.is_quiescent());
}
