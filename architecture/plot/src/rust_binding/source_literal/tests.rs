use super::byte_string_literal;
use crate::prelude::*;

#[test]
fn emitted_byte_literals_compile_and_roundtrip_every_byte_and_escape_boundary() {
    use std::{fs, process::Command};

    let directory = std::env::temp_dir().join(format!(
        "conduit-source-literal-roundtrip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Atomic creation refuses any collision before writing or removing files.
    fs::create_dir(&directory).unwrap();
    let cases = [
        (0..=255).collect::<Vec<u8>>(),
        b"\x0001\x00789abcdefABCDEF\"\\\t\r\n\xff0\x80f\x01F\0\0".to_vec(),
        b"printable ' punctuation ? / and whitespace \t\r\n".to_vec(),
        Vec::new(),
    ];
    let mut source =
        String::from("use std::io::Write; fn main() { let mut out = std::io::stdout().lock();");
    let mut expected = Vec::new();
    for bytes in cases {
        source.push_str("out.write_all(");
        source.push_str(&byte_string_literal(&bytes));
        source.push_str(").unwrap();\n");
        expected.extend(bytes);
    }
    source.push('}');
    let input = directory.join("roundtrip.rs");
    let executable = directory.join("roundtrip");
    fs::write(&input, source).unwrap();
    let compilation = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--edition=2021")
        .arg(&input)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "{}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    let execution = Command::new(&executable).output().unwrap();
    assert!(execution.status.success());
    assert_eq!(execution.stdout, expected);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn printable_and_zero_heavy_payloads_use_compact_single_byte_string() {
    let mut bytes = vec![b'a'; 1024];
    bytes.extend([0; 1024]);
    let literal = byte_string_literal(&bytes);
    assert!(literal.starts_with("b\""));
    assert!(literal.ends_with('"'));
    assert_eq!(literal.len(), 3 + 1024 + 2 * 1024);
    assert!(literal.len() < (3 + bytes.len() * 4) / 2);
    assert_eq!(byte_string_literal(&[]), "b\"\"");
    for digit in b'0'..=b'9' {
        assert_eq!(
            byte_string_literal(&[0, digit]),
            format!("b\"\\x00{}\"", char::from(digit))
        );
    }
}
