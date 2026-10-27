use super::*;

// Preserve the previous recursive traversal as an independent acceptance oracle.
// Typed fields continue to use the production Serde decoder in both cases.
struct Recursive;
impl<'de> serde::Deserialize<'de> for Recursive {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct OldVisitor;
        impl<'de> Visitor<'de> for OldVisitor {
            type Value = Recursive;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON value")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_string<E: de::Error>(self, _: String) -> Result<Recursive, E> {
                Ok(Recursive)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Recursive, A::Error> {
                while seq.next_element::<Recursive>()?.is_some() {}
                Ok(Recursive)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Recursive, A::Error> {
                while map.next_entry::<de::IgnoredAny, Recursive>()?.is_some() {}
                Ok(Recursive)
            }
        }
        decoder.deserialize_any(OldVisitor)
    }
}

fn accepts<T: DeserializeOwned>(bytes: &[u8], chunk: usize, limit: usize) -> bool {
    let mut position = 0;
    from_reader::<T>(
        |out| {
            let count = (bytes.len() - position).min(chunk).min(out.len());
            out[..count].copy_from_slice(&bytes[position..position + count]);
            position += count;
            Ok(count)
        },
        limit,
    )
    .is_ok()
}

#[test]
fn ignored_archive_traversal_preserves_recursive_acceptance_and_refusals() {
    let valid = [
        r#"{"archives":[{"records":[{"id":"a\\b","bytes":[0,255],"extra":null}],"empty":{}},[]],"tail":true}"#,
        r#"[false,null,true,-1,0,18446744073709551615,1.25e-10,"\uD834\uDD1E",{"\u0061":[[]]}]"#,
        r#"{"same":1,"same":2,"escaped":"\"\\\/\b\f\n\r\t"}"#,
        " \n [ {\"x\": [1, 2, 3]}, [], {} ] \t ",
    ];
    for document in valid {
        assert!(accepts::<de::IgnoredAny>(
            document.as_bytes(),
            1,
            document.len()
        ));
        for chunk in [1, 7, 127, 128] {
            for end in 0..=document.len() {
                let bytes = &document.as_bytes()[..end];
                assert_eq!(
                    accepts::<de::IgnoredAny>(bytes, chunk, document.len()),
                    accepts::<Recursive>(bytes, chunk, document.len()),
                    "chunk {chunk}, prefix {end} of {document:?}",
                );
            }
        }
    }
    let invalid = [
        "[1,]",
        "{\"x\":1,}",
        "{1:2}",
        "[1 2]",
        "{\"x\" 1}",
        "[}",
        "{]",
        "[01]",
        "[1e9999]",
        "[NaN]",
        "[tru]",
        "[nullx]",
        r#"["\uD800"]"#,
        r#"["\q"]"#,
        "[\"a\nb\"]",
        "[] true",
        "{}{}",
    ];
    for document in invalid {
        for chunk in [1, 128] {
            assert!(
                !accepts::<de::IgnoredAny>(document.as_bytes(), chunk, document.len()),
                "{document}"
            );
            assert_eq!(
                accepts::<de::IgnoredAny>(document.as_bytes(), chunk, document.len()),
                accepts::<Recursive>(document.as_bytes(), chunk, document.len()),
            );
        }
    }
    let invalid_utf8 = b"{\"x\":\"\xff\"}";
    assert!(!accepts::<de::IgnoredAny>(
        invalid_utf8,
        1,
        invalid_utf8.len()
    ));
}

#[test]
fn ignored_archive_keeps_existing_depth_and_byte_limits() {
    for depth in [1, 126, 127, 128] {
        let document = alloc::format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(
            accepts::<de::IgnoredAny>(document.as_bytes(), 1, document.len()),
            depth <= 127,
        );
        assert_eq!(
            accepts::<de::IgnoredAny>(document.as_bytes(), 7, document.len()),
            accepts::<Recursive>(document.as_bytes(), 7, document.len()),
        );
    }
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[allow(dead_code)]
        archives: de::IgnoredAny,
    }
    for depth in [126, 127] {
        let document = alloc::format!(
            "{{\"archives\":{}0{}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert_eq!(
            accepts::<Envelope>(document.as_bytes(), 1, document.len()),
            depth == 126
        );
        assert!(!accepts::<Envelope>(
            document.as_bytes(),
            1,
            document.len() - 1
        ));
    }
}
