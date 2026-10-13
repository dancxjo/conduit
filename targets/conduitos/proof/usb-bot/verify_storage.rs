//! Independent guest result check against the pinned read-only fixture.
use conduit_core::validate_canonical_structured_value;

pub fn check(log: &str, expected: &[u8]) {
    check_many(log, expected, 1);
}

pub fn check_many(log: &str, expected: &[u8], reads: usize) {
    assert!(matches!(reads, 1 | 16));
    assert_eq!(expected.len(), 512);
    assert!(log.contains("CONDUIT_PROTOCOL_RETIRED complete"));
    assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
    assert!(!log.contains("panicked at"));
    let lines: Vec<_> = log.lines().collect();
    let outputs: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("CONDUIT_PROTOCOL_OUTPUT port=result "))
        .collect();
    assert_eq!(
        outputs.len(),
        reads,
        "exact independently settled block results"
    );
    let retired = lines
        .iter()
        .position(|line| line.starts_with("CONDUIT_PROTOCOL_RETIRED complete"))
        .unwrap();
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("CONDUIT_PROTOCOL_RETIRED complete"))
            .count(),
        1
    );
    for (ordinal, &(position, _)) in outputs.iter().enumerate() {
        let hex = lines[position + 1].trim();
        assert_eq!(hex.len() % 2, 0);
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let value = validate_canonical_structured_value(&bytes).unwrap();
        let completed = value
            .variant_payload("completed")
            .unwrap()
            .expect("completed storage result");
        for (field, number) in [
            ("actual", 512u64),
            ("block_size", 512),
            ("count", 1),
            ("lba", 0),
            ("ordinal", ordinal as u64),
            ("residue", 0),
        ] {
            assert_eq!(
                completed
                    .record_field(field)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                number.to_le_bytes(),
                "{field}"
            );
        }
        assert_eq!(
            completed
                .record_field("short")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            [0]
        );
        assert_eq!(
            completed
                .record_field("bytes")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bytes")
                .unwrap(),
            expected
        );
        assert!(
            retired > position,
            "retirement follows every published settlement"
        );
    }
    println!(
        "{{\"proof_class\":\"emulated-native-block-read\",\"blocks\":{reads},\"bytes_per_block\":512,\"ordered_unique_ordinals\":true,\"retired\":true}}"
    );
}

pub fn check_terminal(log: &str, expected: &str) {
    assert!(matches!(
        expected,
        "stalled" | "provider-lost" | "unsupported" | "timeout" | "command-failed"
    ));
    assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
    assert!(!log.contains("panicked at"));
    let lines: Vec<_> = log.lines().collect();
    let outputs: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("CONDUIT_PROTOCOL_OUTPUT port=result "))
        .collect();
    assert_eq!(outputs.len(), 1);
    let position = outputs[0].0;
    let hex = lines[position + 1].trim();
    assert_eq!(hex.len() % 2, 0);
    let bytes: Vec<_> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let value = validate_canonical_structured_value(&bytes).unwrap();
    assert_eq!(value.variant_tag().unwrap(), expected);
    let identity = value.variant_payload(expected).unwrap().unwrap();
    for (field, number) in [
        ("block_size", 512u64),
        ("count", 1),
        ("lba", 0),
        ("ordinal", 0),
    ] {
        assert_eq!(
            identity
                .record_field(field)
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            number.to_le_bytes()
        );
    }
    let retired: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("CONDUIT_PROTOCOL_RETIRED complete"))
        .collect();
    assert_eq!(retired.len(), 1);
    assert!(retired[0].0 > position);
    println!(
        "{{\"terminal_storage_result\":\"{expected}\",\"ordinal\":0,\"results\":1,\"retired\":true}}"
    );
}
