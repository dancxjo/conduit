use super::*;
use alloc::format;

// Independent legacy transcript construction. Keep this oracle distinct from
// streaming so delimiter, field order and decimal changes cannot self-confirm.
fn legacy(fields: [&str; 7], revision: u64) -> [u8; 32] {
    Sha256::digest(
        format!(
            "conduit.face/mask-plot-show@1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            fields[0], fields[1], fields[2], fields[3], fields[4], revision, fields[5], fields[6],
        )
        .as_bytes(),
    )
    .into()
}

#[test]
fn streamed_show_identity_keeps_legacy_transcript_and_fixed_golden_digests() {
    let absent = [
        "mask-checked",
        "mask-expanded",
        "<none>",
        "<none>",
        "face",
        "mask-plan",
        "manifestation",
    ];
    let unicode = [
        "α\nβ",
        "展開",
        "checked\n",
        "plan",
        "face/🦀",
        "mask-plan",
        "manifestation\n",
    ];
    // Golden digests computed independently from the complete UTF-8 transcript.
    assert_eq!(
        show_digest(absent, 0),
        [
            0xa5, 0x7b, 0x12, 0x07, 0xe0, 0xc9, 0xfb, 0xc2, 0xa4, 0x12, 0x4f, 0x59, 0x2c, 0x9b,
            0xfe, 0x26, 0xd1, 0x41, 0x85, 0x21, 0x1f, 0xb4, 0x59, 0xe3, 0x46, 0xb9, 0xc5, 0x9b,
            0xda, 0x27, 0x36, 0xf4
        ]
    );
    assert_eq!(
        show_digest(unicode, u64::MAX),
        [
            0xa6, 0x90, 0xc6, 0xa4, 0xd7, 0xda, 0xa9, 0xf8, 0xad, 0xff, 0x4a, 0x9c, 0x3a, 0xf7,
            0xea, 0xb7, 0xcf, 0xb6, 0x97, 0x59, 0x78, 0xe1, 0x2b, 0x2c, 0x82, 0x6e, 0x94, 0x06,
            0x5e, 0xc6, 0xd0, 0x86
        ]
    );
    for fields in [
        absent,
        unicode,
        ["m", "e", "checked", "plan", "f", "p", "s"],
        ["", "\n", "<none>", "plan", "é", "p\n", "s"],
    ] {
        for revision in [0, 1, 9, 10, 99, 100, u32::MAX as u64, u64::MAX] {
            assert_eq!(show_digest(fields, revision), legacy(fields, revision));
        }
    }
}

#[test]
fn each_canonical_show_input_still_changes_its_binding() {
    let fields = [
        "mask-checked",
        "mask-expanded",
        "checked",
        "plan",
        "face",
        "mask-plan",
        "manifestation",
    ];
    let base = show_digest(fields, 7);
    for index in 0..fields.len() {
        let mut changed = fields;
        changed[index] = "different";
        assert_ne!(show_digest(changed, 7), base);
    }
    assert_ne!(show_digest(fields, 8), base);
}

#[test]
fn owned_show_id_retains_exact_prefix_and_lowercase_hex_wire_bytes() {
    let fields = [
        "mask-checked",
        "mask-expanded",
        "<none>",
        "<none>",
        "face",
        "mask-plan",
        "manifestation",
    ];
    assert_eq!(
        show_identity(show_digest(fields, 0)).as_str(),
        "show/a57b1207e0c9fbc2a4124f592c9bfe26d14185211fb459e346b9c59bda2736f4"
    );
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let legacy_hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(show_identity(bytes).as_str(), format!("show/{legacy_hex}"));
    assert_eq!(
        show_identity([255; 32]).as_str(),
        format!("show/{}", "ff".repeat(32))
    );
}
