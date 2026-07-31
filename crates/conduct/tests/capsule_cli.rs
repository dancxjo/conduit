use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "conduct-capsule-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_conduct"))
}

#[test]
fn pack_check_inspect_diff_and_unpack_preserve_identity_layers() {
    let root = temporary_directory();
    let panel = root.join("program.panel");
    std::fs::write(&panel, include_str!("../../../examples/hello.panel")).unwrap();
    let presentation_a = root.join("presentation-a.json");
    let presentation_b = root.join("presentation-b.json");
    std::fs::write(&presentation_a, "{\"message\":[0,0]}").unwrap();
    std::fs::write(&presentation_b, "{\"message\":[10,20]}").unwrap();
    let first = root.join("first.cndcapsule.json");
    let second = root.join("second.cndcapsule.json");

    for (presentation, output) in [(&presentation_a, &first), (&presentation_b, &second)] {
        let packed = command()
            .args(["capsule", "pack", "--format=json"])
            .arg(&panel)
            .arg("--presentation")
            .arg(presentation)
            .arg("--output")
            .arg(output)
            .output()
            .unwrap();
        assert!(
            packed.status.success(),
            "{}",
            String::from_utf8_lossy(&packed.stderr)
        );
        assert!(packed.stderr.is_empty());
    }

    let checked = command()
        .args(["capsule", "check", "--format=json"])
        .arg(&first)
        .output()
        .unwrap();
    assert!(checked.status.success());
    let checked: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(checked["operation"], "capsule-check");
    assert_eq!(checked["result"]["root_nodes"], 3);

    let explained = command()
        .args(["capsule", "explain", "--format=json"])
        .arg(&first)
        .output()
        .unwrap();
    assert!(explained.status.success());
    let explained: serde_json::Value = serde_json::from_slice(&explained.stdout).unwrap();
    assert_eq!(explained["operation"], "capsule-explain");
    assert_eq!(explained["result"]["nodes"].as_array().unwrap().len(), 3);

    let diff = command()
        .args(["capsule", "diff", "--format=json"])
        .arg(&first)
        .arg(&second)
        .output()
        .unwrap();
    assert!(diff.status.success());
    let diff: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(diff["result"]["same_program"], true);
    assert_eq!(diff["result"]["same_source_semantics"], true);
    assert_eq!(diff["result"]["same_presentation"], false);
    assert_eq!(diff["result"]["same_capsule"], false);

    let output_dir = root.join("unpacked");
    let unpacked = command()
        .args(["capsule", "unpack", "--format=json"])
        .arg(&first)
        .arg("--output-dir")
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(unpacked.status.success());
    assert_eq!(
        std::fs::read_to_string(output_dir.join("main.panel")).unwrap(),
        include_str!("../../../examples/hello.panel")
    );
    assert!(output_dir.join("presentation.json").is_file());
    assert!(output_dir.join("capsule.json").is_file());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn capsule_tamper_and_existing_output_fail_closed() {
    let root = temporary_directory();
    let panel = root.join("program.panel");
    std::fs::write(&panel, include_str!("../../../examples/hello.panel")).unwrap();
    let capsule = root.join("program.cndcapsule.json");
    let first = command()
        .args(["capsule", "pack"])
        .arg(&panel)
        .arg("--output")
        .arg(&capsule)
        .output()
        .unwrap();
    assert!(first.status.success());
    let overwrite = command()
        .args(["capsule", "pack", "--diagnostic-format=json"])
        .arg(&panel)
        .arg("--output")
        .arg(&capsule)
        .output()
        .unwrap();
    assert!(!overwrite.status.success());
    let diagnostic: serde_json::Value = serde_json::from_slice(&overwrite.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-002");

    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&capsule).unwrap()).unwrap();
    document["source"] = serde_json::Value::String("panel 0\n".to_owned());
    std::fs::write(&capsule, serde_json::to_vec(&document).unwrap()).unwrap();
    let tampered = command()
        .args(["capsule", "inspect", "--diagnostic-format=json"])
        .arg(&capsule)
        .output()
        .unwrap();
    assert!(!tampered.status.success());
    let diagnostic: serde_json::Value = serde_json::from_slice(&tampered.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-CAP-006");
    assert!(tampered.stdout.is_empty());

    std::fs::remove_dir_all(root).unwrap();
}
