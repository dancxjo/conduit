use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn public_formatter_preserves_glyphs_checks_idempotence_and_never_writes_input() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-format-{}-{nonce}.conduit",
        std::process::id()
    ));
    let source = "with text/pattern/notation as r\nplot example {\n\t value = r⟦a#'\",b⟧   \n   }";
    fs::write(&path, source).unwrap();
    let invoke = |check| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
        command.arg("fmt").arg(&path);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    let checked = invoke(true);
    assert!(!checked.status.success());
    assert!(checked.stdout.is_empty());
    let output = invoke(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let formatted = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        formatted,
        "with text/pattern/notation as r\nplot example {\n    value = r⟦a#'\",b⟧\n}\n"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    fs::write(&path, &formatted).unwrap();
    assert!(invoke(true).status.success());
    assert_eq!(String::from_utf8(invoke(false).stdout).unwrap(), formatted);
    let check = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stdout)
    );
    fs::write(&path, "plot unfinished {").unwrap();
    let invalid = invoke(false);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    fs::remove_file(path).unwrap();
}
