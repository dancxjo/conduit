use super::byte_literal;

#[test]
fn template_seal_emission_compiles_all_original_octets_and_keeps_ascii_compact() {
    let bytes: Vec<_> = (0..=u8::MAX).collect();
    let mut literal = String::new();
    byte_literal(&mut literal, &bytes);
    let directory =
        std::env::temp_dir().join(format!("conduit-seal-byte-literal-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("literal.rs");
    std::fs::write(
        &source,
        format!("fn main() {{ let actual:&[u8]={literal}; assert_eq!(actual, &{bytes:?}); }}"),
    )
    .unwrap();
    let executable = directory.join("literal");
    let compiled = std::process::Command::new("rustc")
        .args(["--edition=2021"])
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(std::process::Command::new(executable)
        .status()
        .unwrap()
        .success());
    std::fs::remove_dir_all(directory).unwrap();
    let canonical = br#"{"kind":"conduit/example","value":123}"#;
    let mut compact = String::new();
    byte_literal(&mut compact, canonical);
    assert!(compact.len() < format!("&{canonical:?}").len());
}
