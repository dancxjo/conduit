//! Actual encoder/collector protocol conformance; no native execution is claimed.
extern crate std;
use self::std::{format, string::String};
use crate::{protected_compositor::lifecycle_sign as wire, protected_region::DomainCost};
#[path = "../../make/xtask/compositor_lifecycle_proof.rs"]
mod collector;

const DIGEST: [u8; 32] = [7; 32];
fn identity() -> wire::Identity<'static> {
    wire::Identity {
        host: "host",
        boot: "boot",
        offer_generation: 1,
        base: "display",
        width: 1280,
        height: 800,
        pitch: 5120,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
        make_profile: "profile",
        make_build: "build",
        make_image_binding: "binding",
        guest_image_sha256: &DIGEST,
    }
}
fn expected() -> collector::ExpectedBinding {
    collector::ExpectedBinding {
        source_commit: "a".repeat(40),
        root_elf_sha256: "b".repeat(64),
        iso_sha256: "c".repeat(64),
        preparation: 1,
        service_reserved_bytes: 1000,
        staging_prepared_bytes: 800,
        identity: collector::Identity {
            host: "host".into(),
            boot: "boot".into(),
            offer_generation: 1,
            base: "display".into(),
            width: 1280,
            height: 800,
            pitch: 5120,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
            make_profile: "profile".into(),
            make_build: "build".into(),
            make_image_binding: "binding".into(),
            guest_image_sha256: DIGEST,
        },
    }
}
fn cost(n: u64) -> DomainCost {
    DomainCost {
        entries: n,
        privilege_transitions: n,
        address_space_switches: n,
        scheduler_returns: n,
        reserved_bytes: 1000,
        ..DomainCost::default()
    }
}
fn position(checkpoint: u64, part: u8, parts: u8) -> wire::Position {
    wire::Position {
        preparation: 1,
        checkpoint,
        part,
        parts,
        instance: 9,
        namespace: if checkpoint == 0 {
            None
        } else {
            Some(checkpoint * 3)
        },
    }
}
fn snapshot<'a>(ordinal: u64, part: u8, cost: Option<&'a DomainCost>) -> wire::Snapshot<'a> {
    let prepared = ordinal == 0;
    let (stage, kind) = if prepared {
        [
            (wire::Stage::Preparation, wire::Kind::Context),
            (wire::Stage::Preparation, wire::Kind::Installed),
            (wire::Stage::Preparation, wire::Kind::Before),
            (wire::Stage::Preparation, wire::Kind::After),
            (wire::Stage::Preparation, wire::Kind::Current),
        ][part as usize]
    } else {
        [
            (wire::Stage::Activation, wire::Kind::Context),
            (wire::Stage::Activation, wire::Kind::Before),
            (wire::Stage::Activation, wire::Kind::After),
            (wire::Stage::Request, wire::Kind::Before),
            (wire::Stage::Request, wire::Kind::After),
            (wire::Stage::Request, wire::Kind::Current),
        ][part as usize]
    };
    let paired = if prepared {
        part == 2 || part == 3
    } else {
        (1..=4).contains(&part)
    };
    wire::Snapshot {
        position: position(ordinal, part, if prepared { 5 } else { 6 }),
        checkpoint: [
            wire::Checkpoint::Prepared,
            wire::Checkpoint::InitialHub,
            wire::Checkpoint::FirstCreche,
            wire::Checkpoint::PlayingHub,
        ][ordinal as usize],
        ack: if prepared {
            None
        } else {
            Some(wire::AckRef {
                instance: 9,
                namespace: ordinal * 3,
                frame_sequence: ordinal,
                guest_sequence: ordinal + 20,
            })
        },
        stage,
        kind,
        identity: identity(),
        phase: if paired && !prepared {
            Some(wire::PhaseMetadata {
                count: ordinal * 10 + u64::from(part >= 3),
                overflowed: false,
            })
        } else {
            None
        },
        sample: if paired {
            Some(wire::SampleMetadata {
                succeeded: true,
                refused: false,
                raw_ticks: None,
            })
        } else {
            None
        },
        cost: cost.map(wire::Cost),
        staging_prepared_bytes: 800,
        staging_live_bytes: 800,
        installation_raw_ticks: None,
        staging_preparation_raw_ticks: None,
        preparation_raw_ticks: None,
        disposal_raw_ticks: None,
    }
}
#[test]
fn actual_encoder_complete_batches_join_existing_ack_telemetry() {
    let mut serial = String::new();
    for ordinal in 0..4u64 {
        if ordinal > 0 {
            let n = ordinal * 10 + 4;
            serial.push_str(&format!("CONDUIT_COMPOSITOR_SERVICE_FRAME service_instance=9 namespace={} frame_sequence={ordinal} guest_sequence={} pixels_written=100 damage_count=1 entries={n} privilege_transitions={n} address_space_switches={n} scheduler_returns={n}\n",ordinal*3,ordinal+20));
        }
        for part in 0..if ordinal == 0 { 5 } else { 6 } {
            let n = if ordinal == 0 {
                u64::from(part >= 3)
            } else {
                ordinal * 10 + u64::from(part.min(4))
            };
            let value = cost(n);
            let record = snapshot(ordinal, part, if part == 0 { None } else { Some(&value) });
            let mut buffer = [0; wire::CAPACITY];
            let encoded = wire::encode(&record, &mut buffer);
            assert!(encoded.complete);
            serial.push_str(core::str::from_utf8(&buffer[..encoded.length]).unwrap());
        }
    }
    let report = collector::parse_terminal(&serial, &expected()).unwrap();
    assert!(collector::preparation_context(&serial)
        .unwrap()
        .ack
        .0
        .is_none());
    assert_eq!(report.lifecycle_records, 23);
    assert_eq!(report.checkpoints.len(), 4);
    assert!(!report.artifact_custody_verified);
    assert_eq!(report.instance, 9);
    assert_eq!(report.source_commit, "a".repeat(40));
    assert_eq!(report.root_elf_sha256, "b".repeat(64));
    assert_eq!(report.iso_sha256, "c".repeat(64));
    assert_eq!(
        report.checkpoints[0].checkpoint,
        collector::Checkpoint::Prepared
    );
    assert!(report.checkpoints[0].ack.is_none());
    assert_eq!(report.checkpoints[2].ack.unwrap().namespace, 6);
    assert_eq!(report.checkpoints[3].activation_count, Some(30));
    assert_eq!(report.checkpoints[3].request_count, Some(31));
}
#[test]
fn actual_encoder_overflow_and_capacity_markers_retain_distinct_refusals() {
    let mut record = snapshot(1, 0, None);
    record.phase = Some(wire::PhaseMetadata {
        count: u64::MAX,
        overflowed: true,
    });
    let mut buffer = [0; wire::CAPACITY];
    let encoded = wire::encode(&record, &mut buffer);
    assert!(!encoded.complete);
    assert_eq!(
        collector::parse_terminal(
            core::str::from_utf8(&buffer[..encoded.length]).unwrap(),
            &expected()
        )
        .unwrap_err(),
        collector::Refusal::Incomplete(collector::IncompleteReason::CounterOverflow)
    );
    let long = "x".repeat(wire::CAPACITY);
    record.phase = None;
    record.identity.host = &long;
    let encoded = wire::encode(&record, &mut buffer);
    assert!(!encoded.complete);
    assert_eq!(
        collector::parse_terminal(
            core::str::from_utf8(&buffer[..encoded.length]).unwrap(),
            &expected()
        )
        .unwrap_err(),
        collector::Refusal::Incomplete(collector::IncompleteReason::RecordCapacityOrSerialization)
    );
}

#[test]
fn actual_product_missing_ack_encoder_is_a_distinct_incomplete_packet() {
    // Root integrates the disjoint product-hooks commit before declaring this
    // conformance module; this uses that actual encoder, never fabricated JSON.
    let mut buffer = [0; wire::CAPACITY];
    let encoded = wire::encode_missing_ack(position(1, 0, 6), &mut buffer);
    assert!(!encoded.complete);
    assert_eq!(
        collector::parse_terminal(
            core::str::from_utf8(&buffer[..encoded.length]).unwrap(),
            &expected()
        )
        .unwrap_err(),
        collector::Refusal::Incomplete(collector::IncompleteReason::MissingAckOrRouteRefused)
    );
}
