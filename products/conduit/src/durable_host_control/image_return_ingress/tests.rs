use super::*;
fn bounds() -> ImageBounds {
    ImageBounds {
        maximum_images: 32,
        maximum_compressed_bytes: 24 * 1024,
    }
}
fn begin(images: &str) -> String {
    format!(
        r#"{{"kind":"body-member-image-return","protocol":1,"token":[7],"command":{{"operation":"begin","presentation":{{}},"images":{images}}}}}"#
    )
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Legacy {
    BodyMemberImageReturn {
        protocol: u16,
        token: Vec<u8>,
        command: LegacyCommand,
    },
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum LegacyCommand {
    Begin {
        presentation: serde_json::Value,
        images: Vec<Vec<u8>>,
    },
    Complete {
        proof: serde_json::Value,
    },
}
// This oracle models only framing/byte-array shape. Nested presentation/proof
// semantics are deliberately deferred; actual product differential is separate.
fn legacy(input: &str) -> bool {
    match serde_json::from_str(input) {
        Ok(Legacy::BodyMemberImageReturn {
            protocol,
            token,
            command,
        }) => {
            std::hint::black_box((protocol, token));
            match command {
                LegacyCommand::Begin {
                    presentation,
                    images,
                } => {
                    std::hint::black_box((presentation, images));
                }
                LegacyCommand::Complete { proof } => {
                    std::hint::black_box(proof);
                }
            }
            true
        }
        Err(_) => false,
    }
}
#[test]
fn borrows_spans_and_counts_without_constructing_commands() {
    let frame = begin("[[0,1,255],[]]");
    let view = inspect(&frame, &bounds()).unwrap();
    assert_eq!(view.protocol, 1);
    assert_eq!(view.token_bytes, 1);
    assert_eq!(view.token.get(), "[7]");
    let BorrowedCommand::Begin {
        presentation,
        images,
        layout,
    } = view.command
    else {
        panic!()
    };
    assert_eq!(
        layout,
        ImageLayout {
            images: 2,
            compressed_bytes: 3,
            largest_image: 3
        }
    );
    for span in [view.token, presentation, images] {
        let first = span.get().as_ptr() as usize;
        assert!(first >= frame.as_ptr() as usize);
        assert!(first + span.get().len() <= frame.as_ptr() as usize + frame.len());
    }
}
#[test]
fn framing_shape_matches_legacy_for_order_escaping_duplicates_and_unknowns() {
    let ordinary = begin("[[1]]");
    let reversed = r#"{"command":{"images":[[1]],"presentation":{},"operation":"begin"},"token":[7],"protocol":1,"kind":"body-member-image-return"}"#;
    let escaped = ordinary
        .replace("kind", "ki\\u006ed")
        .replace("begin", "be\\u0067in");
    let complete = r#"{"kind":"body-member-image-return","protocol":1,"token":[],"command":{"operation":"complete","proof":null}}"#;
    for frame in [
        ordinary.clone(),
        reversed.into(),
        escaped,
        complete.into(),
        ordinary.replace("\"protocol\":1", "\"protocol\":1,\"protocol\":1"),
        ordinary.replace("\"images\":[[1]]", "\"images\":[[1]],\"proof\":null"),
        ordinary.replace("\"images\":[[1]]", "\"images\":[[1]],\"images\":[[1]]"),
        ordinary.replace(
            "\"operation\":\"begin\"",
            "\"operation\":\"begin\",\"unknown\":0",
        ),
        ordinary.replace("\"protocol\":1", "\"protocol\":1,\"unknown\":null"),
        ordinary.replace("\"token\":[7]", "\"token\":null"),
        ordinary.replace("\"protocol\":1", "\"protocol\":1.0"),
        ordinary.replace("\"protocol\":1", "\"protocol\":65536"),
        format!("{ordinary} null"),
        format!("{ordinary} []"),
        ordinary[..ordinary.len() - 1].into(),
    ] {
        assert_eq!(
            inspect(&frame, &bounds()).is_ok(),
            legacy(&frame),
            "{frame}"
        );
    }
    let view = inspect(complete, &bounds()).unwrap();
    let BorrowedCommand::Complete { proof } = view.command else {
        panic!()
    };
    assert_eq!(proof.get(), "null");
}
#[test]
fn scalar_byte_acceptance_matches_native_decoder() {
    for images in [
        "[]",
        "[[]]",
        "[[0,1,255]]",
        "[[256]]",
        "[[-1]]",
        "[[-0]]",
        "[[1.0]]",
        "[[1e0]]",
        "[[true]]",
        "[[null]]",
        "[[\"1\"]]",
        "[null]",
        "null",
        "{}",
        "[{}]",
        "[[1],]",
        "[[1]",
        "[[1,]]",
        "[[18446744073709551616]]",
    ] {
        let frame = begin(images);
        assert_eq!(
            inspect(&frame, &bounds()).is_ok(),
            legacy(&frame),
            "{frame}"
        );
    }
}
#[test]
fn selected_bounds_apply_before_owned_image_construction() {
    let frame = begin("[[1,2],[3]]");
    assert!(legacy(&frame));
    assert!(inspect(
        &frame,
        &ImageBounds {
            maximum_images: 1,
            maximum_compressed_bytes: 3
        }
    )
    .is_err());
    assert!(inspect(
        &frame,
        &ImageBounds {
            maximum_images: 2,
            maximum_compressed_bytes: 2
        }
    )
    .is_err());
    assert!(inspect(
        &frame,
        &ImageBounds {
            maximum_images: 2,
            maximum_compressed_bytes: 3
        }
    )
    .is_ok());
    let nested = begin(&format!("[[{}0{}]]", "[".repeat(1000), "]".repeat(1000)));
    assert!(inspect(&nested, &bounds()).is_err());
}

#[test]
fn sequence_forms_and_tag_types_match_legacy_framing() {
    for frame in [
        r#"["body-member-image-return",1,[7],["begin",{},[[1]]]]"#,
        r#"["body-member-image-return",1,[7],["complete",null]]"#,
        r#"["body-member-image-return",1,[7],["complete",null,0]]"#,
        r#"["body-member-image-return",1,[7],["begin",{}]]"#,
        r#"{"kind":{"body-member-image-return":null},"protocol":1,"token":[],"command":{"operation":"complete","proof":null}}"#,
        r#"{"kind":"body-member-image-return","protocol":1,"token":[],"command":{"operation":{"complete":null},"proof":null}}"#,
        r#"{"kind":"body-member-image-return","protocol":1,"token":[],"command":{"operation":0,"proof":null}}"#,
    ] {
        assert_eq!(inspect(frame, &bounds()).is_ok(), legacy(frame), "{frame}");
    }
}

#[test]
fn rejection_diagnostics_never_copy_untrusted_strings() {
    let huge = "sensitive-peer-text-".repeat(8192);
    let quoted = serde_json::to_string(&huge).unwrap();
    let ordinary = begin("[[1]]");
    let cases = [
        quoted.clone(),
        ordinary.replace(
            "\"kind\":\"body-member-image-return\"",
            &format!("\"kind\":{quoted}"),
        ),
        ordinary.replace("\"protocol\":1", &format!("\"protocol\":{quoted}")),
        ordinary.replace("\"token\":[7]", &format!("\"token\":{quoted}")),
        ordinary.replace("\"token\":[7]", &format!("\"token\":[{quoted}]")),
        ordinary.replace("\"command\":{", &format!("\"{huge}\":0,\"command\":{{")),
        ordinary.replace(
            "\"operation\":\"begin\"",
            &format!("\"operation\":{quoted}"),
        ),
        ordinary.replace("\"presentation\":{}", &format!("\"{huge}\":{{}}")),
        ordinary.replace(
            "\"command\":{\"operation\":\"begin\",\"presentation\":{},\"images\":[[1]]}",
            &format!("\"command\":{quoted}"),
        ),
        begin(&quoted),
        begin(&format!("[{quoted}]")),
        begin(&format!("[[{quoted}]]")),
        begin(&format!("[[{}]]", "9".repeat(131072))),
    ];
    for frame in cases {
        let error = match inspect(&frame, &bounds()) {
            Ok(_) => panic!("invalid fixture accepted"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.len() < 256,
            "unbounded diagnostic: {} bytes",
            error.len()
        );
        assert!(!error.contains("sensitive-peer-text"));
        assert!(!legacy(&frame), "legacy acceptance changed");
    }
}

#[test]
fn unsigned_raw_leaf_acceptance_matches_native_without_float_conversion() {
    let huge_integer = "9".repeat(131072);
    let huge_mantissa = format!("0.{}1", "0".repeat(131072));
    let huge_exponent = format!("1e{}", "9".repeat(131072));
    for input in [
        "0",
        "255",
        "256",
        "65535",
        "65536",
        "18446744073709551615",
        "18446744073709551616",
        "-0",
        "-1",
        "0.0",
        "1.0",
        "1e0",
        "0e999999",
        " 255 ",
        "\n65535\t",
        "01",
        "00",
        "+1",
        "1true",
        "1 2",
        "1e+",
        "1.",
        "null",
        "true",
        "\"1\"",
        "[]",
        "{}",
        "[1]",
        "",
        " ",
        &huge_integer,
        &huge_mantissa,
        &huge_exponent,
    ] {
        assert_eq!(
            serde_json::from_str::<Unsigned<255>>(input).is_ok(),
            serde_json::from_str::<u8>(input).is_ok(),
            "u8 {input:.80}"
        );
        assert_eq!(
            serde_json::from_str::<Unsigned<65535>>(input).is_ok(),
            serde_json::from_str::<u16>(input).is_ok(),
            "u16 {input:.80}"
        );
        let image_frame = begin(&format!("[[{input}]]"));
        assert_eq!(
            inspect(&image_frame, &bounds()).is_ok(),
            legacy(&image_frame),
            "image leaf {input:.80}"
        );
        let protocol_frame =
            begin("[[1]]").replace("\"protocol\":1", &format!("\"protocol\":{input}"));
        assert_eq!(
            inspect(&protocol_frame, &bounds()).is_ok(),
            legacy(&protocol_frame),
            "protocol leaf {input:.80}"
        );
    }
}

#[test]
fn invalid_numeric_roots_and_tags_are_refused_before_float_conversion() {
    let ordinary = begin("[[1]]");
    let mantissa = format!("1.{}e-1", "1234567890".repeat(8192));
    let exponent = format!("1e{}", "9".repeat(131072));
    let integer = "9".repeat(131072);
    for number in [&mantissa, &exponent, &integer] {
        let cases = [
            number.clone(),
            ordinary.replace(
                "\"kind\":\"body-member-image-return\"",
                &format!("\"kind\":{number}"),
            ),
            ordinary.replace(
                "\"command\":{\"operation\":\"begin\",\"presentation\":{},\"images\":[[1]]}",
                &format!("\"command\":{number}"),
            ),
            ordinary.replace(
                "\"operation\":\"begin\"",
                &format!("\"operation\":{number}"),
            ),
            ordinary.replace("\"token\":[7]", &format!("\"token\":{number}")),
            begin(number),
            begin(&format!("[{number}]")),
        ];
        for frame in cases {
            let error = match inspect(&frame, &bounds()) {
                Ok(_) => panic!("numeric shape accepted"),
                Err(error) => error.to_string(),
            };
            assert!(
                error.starts_with("unexpected image-return JSON shape"),
                "{error}"
            );
            assert!(error.len() < 256);
            assert!(!legacy(&frame));
        }
    }
}
