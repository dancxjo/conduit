use conduit_core::validate_canonical_structured_value;
fn main() {
    let mut args = std::env::args().skip(1);
    let log = std::fs::read_to_string(args.next().expect("serial log")).unwrap();
    let port = args.next().unwrap_or_else(|| "result".into());
    let receive_hex = args
        .next()
        .unwrap_or_else(|| "6364632d6465766963652d7265706c790a".into());
    assert_eq!(receive_hex.len() % 2, 0);
    let receive: Vec<u8> = receive_hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert!(receive.len() < 64);
    assert!(log.contains("CONDUIT_PROTOCOL_RETIRED complete"));
    assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
    let lines: Vec<_> = log.lines().collect();
    let mut count = 0u64;
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with(&format!("CONDUIT_PROTOCOL_OUTPUT port={port} ")) {
            continue;
        }
        let hex = lines[index + 1].trim();
        assert_eq!(hex.len() % 2, 0);
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let value = validate_canonical_structured_value(&bytes).unwrap();
        let tag = if count % 2 == 0 {
            "transmitted"
        } else {
            "received"
        };
        let result = value
            .variant_payload(tag)
            .unwrap()
            .expect("expected ordered successful result");
        assert_eq!(
            result
                .record_field("ordinal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            count.to_le_bytes()
        );
        let requested = if count % 2 == 0 { 12u64 } else { 64u64 };
        let actual = if count % 2 == 0 {
            12u64
        } else {
            receive.len() as u64
        };
        assert_eq!(
            result
                .record_field("requested")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            requested.to_le_bytes()
        );
        assert_eq!(
            result
                .record_field("actual")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            actual.to_le_bytes()
        );
        if count % 2 == 1 {
            assert_eq!(
                result
                    .record_field("short")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap(),
                [1]
            );
            assert_eq!(
                result
                    .record_field("bytes")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap(),
                receive.as_slice()
            );
        }
        count += 1;
    }
    assert_eq!(count, 32);
    println!(
        "{{\"serial_results\":32,\"transmitted\":16,\"received\":16,\"ordered\":true,\"retired\":true}}"
    );
}
