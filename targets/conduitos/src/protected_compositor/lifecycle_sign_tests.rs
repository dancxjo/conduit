use super::*;
const DIGEST: [u8; 32] = [u8::MAX; 32];

fn identity(value: &str) -> Identity<'_> {
    Identity {
        host: value,
        boot: value,
        offer_generation: u64::MAX,
        base: value,
        width: u32::MAX,
        height: u32::MAX,
        pitch: u32::MAX,
        bits_per_pixel: u8::MAX,
        red_shift: u8::MAX,
        green_shift: u8::MAX,
        blue_shift: u8::MAX,
        make_profile: value,
        make_build: value,
        make_image_binding: value,
        guest_image_sha256: &DIGEST,
    }
}
fn snapshot<'a>(value: &'a str, cost: &'a DomainCost) -> Snapshot<'a> {
    Snapshot {
        position: Position {
            preparation: u64::MAX,
            checkpoint: u64::MAX,
            part: u8::MAX - 1,
            parts: u8::MAX,
            instance: u64::MAX,
            namespace: Some(u64::MAX),
        },
        stage: Stage::Preparation,
        kind: Kind::Context,
        identity: identity(value),
        phase: Some(PhaseMetadata {
            count: u64::MAX,
            overflowed: false,
        }),
        sample: Some(SampleMetadata {
            succeeded: true,
            refused: false,
            raw_ticks: Some(u64::MAX),
        }),
        cost: Some(Cost(cost)),
        staging_prepared_bytes: u64::MAX,
        staging_live_bytes: u64::MAX,
        installation_raw_ticks: Some(u64::MAX),
        staging_preparation_raw_ticks: Some(u64::MAX),
        preparation_raw_ticks: Some(u64::MAX),
        disposal_raw_ticks: Some(u64::MAX),
    }
}
fn parse(buffer: &[u8; CAPACITY], encoded: Encoded) -> serde_json::Value {
    assert!(encoded.length <= CAPACITY);
    assert_eq!(&buffer[..PREFIX.len()], PREFIX);
    assert_eq!(buffer[encoded.length - 1], b'\n');
    assert!(buffer[encoded.length..].iter().all(|b| *b == 0));
    serde_json::from_slice(&buffer[PREFIX.len()..encoded.length - 1]).unwrap()
}
#[test]
fn maximum_numeric_cost_fields_and_escaped_identity_are_lossless() {
    let cost = DomainCost {
        entries: u64::MAX,
        interrupt_entries: u64::MAX,
        source_timer_interrupts: u64::MAX,
        privilege_transitions: u64::MAX,
        gate_transitions: u64::MAX,
        copied_bytes: u64::MAX,
        setup_copied_bytes: u64::MAX,
        base_gate_transitions: u64::MAX,
        tlb_flushes: u64::MAX,
        setup_ticks: u64::MAX,
        teardown_ticks: u64::MAX,
        teardown_zeroed_bytes: u32::MAX,
        shared_peak_bytes: u32::MAX,
        root_metadata_bytes: u32::MAX,
        address_space_switches: u64::MAX,
        scheduler_returns: u64::MAX,
        preemptions: u64::MAX,
        reserved_bytes: u32::MAX,
    };
    let value = "quoted\" slash\\ line\n tab\t return\r nul\0 backspace\u{8} emoji🦀";
    let record = snapshot(value, &cost);
    let mut buffer = [0; CAPACITY];
    let encoded = encode(&record, &mut buffer);
    assert!(encoded.complete);
    let json = parse(&buffer, encoded);
    assert_eq!(json["snapshot"]["identity"]["host"], value);
    assert_eq!(json["snapshot"]["identity"]["make_image_binding"], value);
    let projected = json["snapshot"]["cost"].as_object().unwrap();
    assert_eq!(projected.len(), 18);
    for (name, value) in projected {
        let max = if [
            "teardown_zeroed_bytes",
            "shared_peak_bytes",
            "root_metadata_bytes",
            "reserved_bytes",
        ]
        .contains(&name.as_str())
        {
            u32::MAX as u64
        } else {
            u64::MAX
        };
        assert_eq!(value.as_u64(), Some(max), "{name}");
    }
}
#[test]
fn missing_samples_cost_and_ticks_remain_explicit_null_for_every_closed_kind() {
    let cost = DomainCost::default();
    for stage in [
        Stage::Preparation,
        Stage::Activation,
        Stage::Request,
        Stage::Disposal,
    ] {
        for kind in [
            Kind::Context,
            Kind::Installed,
            Kind::Before,
            Kind::After,
            Kind::Current,
        ] {
            let mut record = snapshot("scope", &cost);
            record.stage = stage;
            record.kind = kind;
            record.sample = None;
            record.phase = None;
            record.cost = None;
            record.preparation_raw_ticks = None;
            let mut buffer = [0; CAPACITY];
            let encoded = encode(&record, &mut buffer);
            assert!(encoded.complete);
            let json = parse(&buffer, encoded);
            for field in ["sample", "phase", "cost", "preparation_raw_ticks"] {
                assert!(json["snapshot"][field].is_null(), "{field}");
            }
        }
    }
}
#[test]
fn oversized_identity_and_counter_overflow_emit_only_complete_incomplete_marker() {
    let cost = DomainCost::default();
    let long = "\"\\\n".repeat(CAPACITY);
    let mut record = snapshot(&long, &cost);
    let mut buffer = [255; CAPACITY];
    let encoded = encode(&record, &mut buffer);
    assert!(!encoded.complete);
    let json = parse(&buffer, encoded);
    assert_eq!(json["record_kind"], "incomplete");
    assert_eq!(json["reason"], "record-capacity-or-serialization");
    assert!(json.get("snapshot").is_none());
    record.phase.as_mut().unwrap().overflowed = true;
    let encoded = encode(&record, &mut buffer);
    assert!(!encoded.complete);
    let json = parse(&buffer, encoded);
    assert_eq!(json["reason"], "counter-overflow");
    assert_eq!(json["position"]["instance"], u64::MAX);
}

#[test]
fn growing_escaped_identity_never_exposes_a_truncated_record() {
    let cost = DomainCost::default();
    let mut reached_capacity = false;
    for length in [0, 1, 16, 64, 128, 256, 512, 1024, CAPACITY] {
        let value = "\"".repeat(length);
        let record = snapshot(&value, &cost);
        let mut buffer = [0; CAPACITY];
        let encoded = encode(&record, &mut buffer);
        let json = parse(&buffer, encoded);
        if encoded.complete {
            assert!(!reached_capacity);
            assert_eq!(json["snapshot"]["identity"]["base"], value);
        } else {
            reached_capacity = true;
            assert_eq!(json["complete"], false);
            assert!(json.get("snapshot").is_none());
        }
    }
    assert!(reached_capacity);
}

#[test]
fn guest_image_digest_retains_every_borrowed_byte_without_hex_allocation() {
    let cost = DomainCost::default();
    let digest: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(17));
    let mut record = snapshot("scope", &cost);
    record.identity.guest_image_sha256 = &digest;
    assert!(core::ptr::eq(record.identity.guest_image_sha256, &digest));
    let mut buffer = [0; CAPACITY];
    let encoded = encode(&record, &mut buffer);
    assert!(encoded.complete);
    let json = parse(&buffer, encoded);
    let actual = json["snapshot"]["identity"]["guest_image_sha256"]
        .as_array()
        .unwrap();
    assert_eq!(actual.len(), 32);
    for (byte, expected) in actual.iter().zip(digest) {
        assert_eq!(byte.as_u64(), Some(u64::from(expected)));
    }
}

#[test]
fn prepared_record_and_incomplete_paths_make_no_heap_allocation_calls() {
    // Preparation and JSON parsing are deliberately outside the measured closure.
    let cost = DomainCost {
        entries: u64::MAX,
        setup_ticks: u64::MAX,
        teardown_zeroed_bytes: u32::MAX,
        reserved_bytes: u32::MAX,
        ..DomainCost::default()
    };
    let normal = snapshot("escaped\"\\\n", &cost);
    let long = "\"\\\n".repeat(CAPACITY);
    let oversized = snapshot(&long, &cost);
    let mut overflowed = snapshot("scope", &cost);
    overflowed.phase.as_mut().unwrap().overflowed = true;
    let records = [&normal, &oversized, &overflowed];
    let mut buffers = [[0; CAPACITY]; 3];
    let mut encoded = [Encoded {
        length: 0,
        complete: false,
    }; 3];
    let (calls, retained_bytes) = crate::test_allocations::rounded_retained_bytes(|| {
        crate::test_allocations::allocations(|| {
            for i in 0..records.len() {
                encoded[i] = encode(records[i], &mut buffers[i]);
            }
        })
    });
    // The helper counts alloc, alloc_zeroed and realloc calls, including failed
    // attempts: zero calls means no heap storage was requested by emission.
    assert_eq!(calls, 0);
    // This additional helper reports net rounded retained bytes, not peak/total.
    assert_eq!(retained_bytes, 0);
    assert!(encoded[0].complete);
    assert!(!encoded[1].complete);
    assert!(!encoded[2].complete);
    for i in 0..records.len() {
        let json = parse(&buffers[i], encoded[i]);
        assert_eq!(json["complete"], encoded[i].complete);
    }
}
