//! Exercise the actual service CLI: diagnostics cannot change a startup refusal.
use super::*;
use std::process::Stdio;

const PREFIX: &str = "CONDUIT_HOST_STARTUP_PHASE ";

#[test]
fn startup_trace_is_exactly_opt_in_and_preserves_the_real_cli_refusal() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "conduit-startup-private-fixture-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let installation = b"private-fixture-invalid-installation\n";
    fs::write(root.join("installation.json"), installation).unwrap();
    let mut baseline = None;
    for enabled in [None, Some("0"), Some("true"), Some("1")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
        command
            .args(["host", "service", "run", "--state-dir"])
            .arg(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_remove("CONDUIT_HOST_STARTUP_TRACE");
        if let Some(value) = enabled {
            command.env("CONDUIT_HOST_STARTUP_TRACE", value);
        }
        let child = command.spawn().unwrap();
        let pid = child.id();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!root.join("runtime.json").exists());
        assert_eq!(
            fs::read(root.join("installation.json")).unwrap(),
            installation
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        let mut records = Vec::new();
        let mut original = String::new();
        for line in stderr.split_inclusive('\n') {
            if let Some(json) = line.strip_prefix(PREFIX) {
                assert!(line.len() <= 512);
                assert!(!line.contains("private-fixture"));
                assert!(!line.contains(root.to_str().unwrap()));
                records.push(serde_json::from_str::<Value>(json).unwrap());
            } else {
                original.push_str(line);
            }
        }
        let refusal = (output.status.code(), original);
        if let Some(expected) = &baseline {
            assert_eq!(&refusal, expected);
        } else {
            baseline = Some(refusal);
        }
        if enabled != Some("1") {
            assert!(records.is_empty());
            continue;
        }
        assert!((3..=40).contains(&records.len()));
        let mut elapsed = 0;
        for (sequence, record) in records.iter().enumerate() {
            assert_eq!(record.as_object().unwrap().len(), 7);
            assert_eq!(record["schema"], "conduit.proof/host-startup@1");
            assert_eq!(record["pid"], pid);
            assert_eq!(record["sequence"], sequence);
            let now = record["elapsed_us"].as_u64().unwrap();
            assert!(now >= elapsed);
            elapsed = now;
        }
        assert_eq!(records[0]["event"], "start");
        assert_eq!(records.last().unwrap()["event"], "aborted");
        assert!(records.iter().any(|r| r["event"] == "refused"));
        assert!(records.iter().all(|r| r["event"] != "prepared"));
    }
    fs::remove_dir_all(root).unwrap();
}
