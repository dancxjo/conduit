//! Decode retained native discovery through Core's canonical validator.
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue as Value};

fn field<'a>(value: Value<'a>, name: &str) -> Value<'a> {
    value.record_field(name).unwrap().expect("required carrier field")
}
fn number(value: Value<'_>) -> u64 {
    u64::from_le_bytes(value.primitive_bytes("value/u64").unwrap().try_into().unwrap())
}
fn geometry(value: Value<'_>) -> String {
    match value.variant_tag().unwrap() {
        "absent" => {
            assert!(value.variant_payload("absent").unwrap().unwrap().primitive_bytes("value/empty").unwrap().is_empty());
            "null".into()
        }
        "selected" => {
            let body = value.variant_payload("selected").unwrap().unwrap();
            format!("{{\"address\":{},\"endpoint_epoch\":{},\"interval\":{},\"maximum_packet\":{},\"transfer_type\":{}}}",
                number(field(body, "address")), number(field(body, "epoch")),
                number(field(body, "interval")), number(field(body, "packet")), number(field(body, "transfer")))
        }
        _ => panic!("unknown discovery selection"),
    }
}
fn main() {
    let log = std::fs::read_to_string(std::env::args().nth(1).expect("serial transcript")).unwrap();
    let lines: Vec<_> = log.lines().collect();
    let observations: Vec<_> = lines.iter().enumerate().filter(|(_, line)| line.starts_with("CONDUIT_USB_PREFLIGHT_OBSERVATION ")).collect();
    assert_eq!(observations.len(), 1);
    let index = observations[0].0;
    let hex = lines[index + 1];
    assert_eq!(hex.len() % 2, 0);
    assert!(hex.len() <= 8192);
    let bytes: Vec<u8> = hex.as_bytes().chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
    let carrier = validate_canonical_structured_value(&bytes).unwrap();
    let completion = field(carrier, "configuration").variant_payload("completed").unwrap().expect("actual descriptor completion");
    let transferred = u16::from_le_bytes(field(completion, "transferred").primitive_bytes("value/u16").unwrap().try_into().unwrap());
    let short = field(completion, "short").primitive_bytes("value/bool").unwrap();
    assert!(short == [0] || short == [1]);
    let input = field(completion, "input");
    let mut descriptor = String::new();
    let mut actual = 0u16;
    for index in 0..=256 {
        let Some(value) = input.collection_index(index).unwrap() else { break; };
        assert!(index < 256);
        let byte = value.primitive_bytes("value/u8").unwrap();
        assert_eq!(byte.len(), 1);
        descriptor.push_str(&format!("{:02x}", byte[0]));
        actual += 1;
    }
    assert_eq!(actual, transferred);
    println!("{{\"attachment_epoch\":{},\"configuration\":{{\"transferred\":{},\"short\":{},\"input_hex\":\"{}\"}},\"endpoint_read\":{},\"endpoint_write\":{},\"canonical_core_validation\":true}}",
        number(field(carrier, "attachment")), transferred, short == [1], descriptor,
        geometry(field(carrier, "read")), geometry(field(carrier, "write")));
}
