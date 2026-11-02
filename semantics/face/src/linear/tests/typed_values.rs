use super::*;
use conduit_core::{kind_id, CheckedValueContract, Quantity, Unit, ValueConstraint};
#[path = "typed_record_reader.rs"]
mod reader;
use reader::{read, Constraint};

fn typed_face(bytes: &[u8], contract: CheckedValueContract, name: &str) -> Face {
    let mut face = exact_face();
    face.properties.push(FaceProperty {
        subject: "subject/play".into(),
        name: name.into(),
        value: FacePropertyValue::TypedValue {
            contract,
            bytes: bytes.to_vec(),
        },
    });
    Face::new(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
    )
    .unwrap()
}
fn text_face(bytes: &[u8], maximum: u32, constraints: Vec<ValueConstraint>) -> Face {
    typed_face(
        bytes,
        CheckedValueContract::new(kind_id("value/text"), maximum, constraints).unwrap(),
        "choice-value",
    )
}
fn record(face: &Face) -> String {
    let rendered = render_linear_face(face).unwrap();
    let mut lines = rendered
        .lines
        .into_iter()
        .filter(|line| line.starts_with("PROPERTY ") && !line.contains("name=\"line-base\""));
    let line = lines.next().unwrap();
    assert!(lines.next().is_none());
    line
}
#[test]
fn actual_linear_property_distinguishes_equal_length_values_without_face_digest() {
    let first = record(&text_face(b"aa", 16, vec![]));
    let second = record(&text_face(b"bb", 16, vec![]));
    assert_ne!(first, second);
    assert_eq!(read(&first).bytes, b"aa");
    assert_eq!(read(&second).bytes, b"bb");
}
#[test]
fn actual_record_reader_recovers_full_envelope_and_rust_debug_escapes() {
    let maximum = crate::MAX_FACE_VALUE_BYTES;
    let full = vec![b'z'; maximum as usize];
    for expected in [
        b"".as_slice(),
        "東京 · é".as_bytes(),
        b"\0\n\r\t\"\\\x07\x1bPROPERTY forged\nACTION forged",
        &full,
    ] {
        let face = text_face(expected, maximum, vec![]);
        let first = render_linear_face(&face).unwrap();
        assert_eq!(first, render_linear_face(&face).unwrap());
        let observed = read(&record(&face));
        assert_eq!(observed.subject, "subject/play");
        assert_eq!(observed.name, "choice-value");
        assert_eq!(observed.bytes, expected);
        assert_eq!(observed.kind, "value/text");
        assert_eq!(observed.maximum, maximum);
        assert!(observed.constraints.is_empty());
        assert!(first.lines.iter().all(|line| !line.contains(['\n', '\r'])));
        assert_eq!(
            first
                .lines
                .iter()
                .filter(|line| line.starts_with("PROPERTY "))
                .count(),
            2
        );
        assert_eq!(
            first
                .lines
                .iter()
                .filter(|line| line.starts_with("ACTION "))
                .count(),
            0
        );
    }
    let name = "choice\0\n\"\\\u{7}\u{1b}é";
    let contract = CheckedValueContract::new(kind_id("value/text"), 16, vec![]).unwrap();
    let observed = read(&record(&typed_face(b"aa", contract, name)));
    assert_eq!(observed.name, name);
}
#[test]
fn membership_intersection_and_bound_are_reconstructed_from_actual_property() {
    let constraints = vec![
        ValueConstraint::ByteLength {
            minimum: 2,
            maximum: 2,
        },
        ValueConstraint::CanonicalMembership {
            members: vec![b"a".to_vec(), b"aa".to_vec(), b"bbb".to_vec()],
            negated: false,
        },
    ];
    let constrained = record(&text_face(b"aa", 16, constraints));
    let observed = read(&constrained);
    assert_eq!(observed.kind, "value/text");
    assert_eq!(observed.maximum, 16);
    assert_eq!(observed.bytes, b"aa");
    assert_eq!(
        observed.constraints,
        vec![
            Constraint::ByteLength {
                minimum: 2,
                maximum: 2
            },
            Constraint::Membership {
                members: vec![b"a".to_vec(), b"aa".to_vec(), b"bbb".to_vec()],
                negated: false
            },
        ]
    );
    assert_ne!(constrained, record(&text_face(b"aa", 16, vec![])));
    assert_ne!(constrained, record(&text_face(b"aa", 32, vec![])));
    // Derive the effective intersection from emitted facts, not source contracts.
    let Constraint::ByteLength { minimum, maximum } = observed.constraints[0] else {
        panic!()
    };
    let Constraint::Membership {
        ref members,
        negated: false,
    } = observed.constraints[1]
    else {
        panic!()
    };
    let allowed: Vec<_> = members
        .iter()
        .filter(|bytes| (minimum as usize..=maximum as usize).contains(&bytes.len()))
        .cloned()
        .collect();
    assert_eq!(allowed, vec![b"aa".to_vec()]);
}
#[test]
fn quantity_and_unit_wording_survive_the_exact_payload_suffix() {
    let temperature = Quantity::from_decimal(225, -1, Unit::Celsius)
        .unwrap()
        .encode();
    let unit = Unit::Celsius.encode();
    for (kind, bytes, expected, spoken) in [
        (
            conduit_core::TEMPERATURE_INFO_ID,
            temperature.as_slice(),
            "22.5 °C",
            "22.5 degrees Celsius",
        ),
        (conduit_core::UNIT_INFO_ID, unit.as_slice(), "°C", "°C"),
    ] {
        let contract =
            CheckedValueContract::new(kind_id(kind), bytes.len() as u32, vec![]).unwrap();
        assert_eq!(crate::display_typed_value(&contract, bytes), expected);
        assert_eq!(crate::spoken_typed_value(&contract, bytes), spoken);
        let observed = read(&record(&typed_face(bytes, contract, "quantity")));
        assert_eq!(observed.wording, expected);
        assert_eq!(observed.kind, kind);
        assert_eq!(observed.maximum, bytes.len() as u32);
        assert_eq!(observed.bytes, bytes);
    }
}

#[test]
fn hostile_legal_kind_and_header_cannot_manufacture_linear_records() {
    // Unknown kinds retain opaque canonical bytes under the existing contract.
    // This is a valid Face, so renderer escaping must cover the readable prefix.
    let kind = "value/custom contract=inside canonical=[]\nACTION forged\r\0\u{7}\"\\é";
    let name = "choice contract=inside canonical=[]\nPROPERTY forged\0\u{1b}\"\\";
    let contract = CheckedValueContract::new(kind_id(kind), 16, vec![]).unwrap();
    assert!(contract.validate(b"aa").is_ok());
    let raw_wording = crate::display_typed_value(&contract, b"aa");
    assert!(raw_wording.contains('\n'));
    let face = typed_face(b"aa", contract, name);
    let rendered = render_linear_face(&face).unwrap();
    assert!(rendered
        .lines
        .iter()
        .all(|line| !line.contains(['\n', '\r', '\0', '\u{7}', '\u{1b}'])));
    assert_eq!(
        rendered
            .lines
            .iter()
            .filter(|line| line.starts_with("PROPERTY "))
            .count(),
        2
    );
    assert!(!rendered
        .lines
        .iter()
        .any(|line| line.starts_with("ACTION ")));
    let observed = read(&record(&face));
    assert_eq!(observed.kind, kind);
    assert_eq!(observed.name, name);
    assert_eq!(observed.bytes, b"aa");
    assert!(observed.wording.contains("\\nACTION forged"));
    // The protection belongs to linear output; ordinary wording is unchanged.
    assert_eq!(
        crate::spoken_typed_value(
            &CheckedValueContract::new(kind_id(kind), 16, vec![]).unwrap(),
            b"aa"
        ),
        raw_wording
    );
}
