use conduit_core::validate_canonical_structured_value;
fn main() {
    let mut args = std::env::args().skip(1);
    let log = std::fs::read_to_string(args.next().expect("serial log")).unwrap();
    let port = args.next().unwrap_or_else(|| "result".into());
    let receive_hex = args.next().unwrap_or_else(|| "6364632d6465766963652d7265706c790a".into());
    assert_eq!(receive_hex.len() % 2, 0);
    let receive: Vec<u8> = receive_hex.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
    assert!(receive.len() < 64);
    let diagnostic = args.next().as_deref() == Some("budget-exhausted");
    if diagnostic {
        assert!(!log.contains("CONDUIT_PROTOCOL_RETIRED complete"));
        assert!(log.contains("CONDUIT_PROTOCOL_REFUSAL protocol-step-budget-exhausted"));
    } else {
        assert!(log.contains("CONDUIT_PROTOCOL_RETIRED complete"));
        assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
    }
    let lines: Vec<_> = log.lines().collect();
    let mut count = 0u64;
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with(&format!("CONDUIT_PROTOCOL_OUTPUT port={port} ")) { continue; }
        let hex = lines[index + 1].trim();
        assert_eq!(hex.len() % 2, 0);
        let bytes: Vec<u8> = hex.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
        let value = validate_canonical_structured_value(&bytes).unwrap();
        assert_eq!(value.variant_tag().unwrap(), "unsupported");
        assert!(value.variant_payload("unsupported").unwrap().unwrap().primitive_bytes("value/empty").unwrap().is_empty());
        count += 1;
    }
    assert_eq!(count, 1);
    println!("{{\"serial_results\":1,\"terminal\":\"unsupported\",\"no_serial_success\":true,\"retired\":{}}}", !diagnostic);
}
