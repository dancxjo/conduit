//! A public zero-Body client keeps the one service-owned Body after Birth.

#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Service(Child);

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn zero_body_client_refuses_two_plots_then_continues_one_retained_body() {
    let state = std::env::temp_dir().join(format!(
        "conduit-installed-birth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    let mut service = start_service(&state);

    let before = product(&["body", "face", "--state-dir", path(&state), "--json"]);
    assert!(
        !before.status.success(),
        "zero-Body Host advertised an owned Body Face"
    );

    let two_plots = b"focus creche.name\nedit value Ada\nactivate\nfocus creche.plot.0\nedit value true\nactivate\nfocus creche.plot.1\nedit value true\nactivate\nfocus creche.birth\nactivate\nquit\n";
    let refused = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        two_plots,
    );
    assert!(
        refused.status.success(),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(String::from_utf8_lossy(&refused.stdout)
        .contains("installed-birth-multiple-plots-unsupported"));
    assert!(!state.join("body/biography.json").exists());

    let one_plot = b"focus creche.plot.0\nedit value false\nactivate\nfocus creche.birth\nactivate\nread all\nquit\n";
    let session = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        one_plot,
    );
    assert!(
        session.status.success(),
        "Birth client failed: {}\n{}",
        String::from_utf8_lossy(&session.stderr),
        String::from_utf8_lossy(&session.stdout)
    );
    let spoken = String::from_utf8(session.stdout).unwrap();
    assert!(spoken.contains("Body retained by this installed Host:"));
    assert!(spoken.contains("Continuing retained Body"));
    assert!(spoken.contains("Text Face revision="));

    let biography: Value =
        serde_json::from_slice(&fs::read(state.join("body/biography.json")).unwrap()).unwrap();
    let body_id = biography["body_id"].as_str().unwrap();
    assert_eq!(
        biography["body"]["workset"]["plots"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(spoken.contains(body_id));

    let face = product(&["body", "face", "--state-dir", path(&state), "--json"]);
    assert!(
        face.status.success(),
        "{}",
        String::from_utf8_lossy(&face.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&face.stdout).unwrap();
    assert_eq!(snapshot.as_object().unwrap().len(), 3);
    assert_eq!(snapshot["schema"], "conduit.body/local-face-snapshot@1");
    assert_eq!(snapshot["presentation"]["basis"]["body_id"], body_id);
    assert_eq!(
        snapshot["advertisement"]["host_id"],
        biography["membership"]["parts"][0]["current"]["host_id"]
    );

    let started = product(&[
        "body",
        "start",
        "--state-dir",
        path(&state),
        "--maximum-millis",
        "1000",
    ]);
    assert!(
        started.status.success(),
        "retained Clock did not admit Play: {}",
        String::from_utf8_lossy(&started.stderr)
    );
    assert!(
        String::from_utf8_lossy(&started.stdout).contains("conduit.body/service-run-requested@1")
    );
    let lull = product(&["body", "lull", "--state-dir", path(&state)]);
    assert!(
        lull.status.success(),
        "{}",
        String::from_utf8_lossy(&lull.stderr)
    );

    let repeated = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        b"quit\n",
    );
    assert!(
        !repeated.status.success(),
        "a retained Body opened a second Birth"
    );

    stop_service(&mut service);
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let recovered = product(&["body", "face", "--state-dir", path(&state), "--json"]);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let recovered: Value = serde_json::from_slice(&recovered.stdout).unwrap();
    assert_eq!(recovered["presentation"]["basis"]["body_id"], body_id);
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn interrupted_birth_publication_is_unknown_and_recovers_without_a_second_birth() {
    let state = std::env::temp_dir().join(format!(
        "conduit-interrupted-birth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    // The journal can be written, but publication of its biography cannot
    // replace a directory. This models a failure after the commit decision.
    fs::create_dir_all(state.join("body/biography.json")).unwrap();
    let mut service = start_service(&state);
    let attempt = product_with_stdin(
        &["body", "birth", "--screen-free", "--state-dir", path(&state)],
        b"focus creche.name\nedit value Ada\nactivate\nfocus creche.plot.1\nedit value true\nactivate\nfocus creche.birth\nactivate\n",
    );
    assert!(!attempt.status.success());
    assert!(String::from_utf8_lossy(&attempt.stderr).contains("control-outcome-unknown"));
    let journal = state.join("body/owner-transaction.json");
    let transaction: Value = serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    let body_id = transaction["biography"]["body_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let repeated = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        b"quit\n",
    );
    assert!(!repeated.status.success());
    assert!(String::from_utf8_lossy(&repeated.stderr).contains("control-outcome-unknown"));

    stop_service(&mut service);
    fs::remove_dir(state.join("body/biography.json")).unwrap();
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let recovered = product(&["body", "face", "--state-dir", path(&state), "--json"]);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let recovered: Value = serde_json::from_slice(&recovered.stdout).unwrap();
    assert_eq!(recovered["presentation"]["basis"]["body_id"], body_id);
    assert!(!journal.exists());
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

/// Local device evidence only: the receipt proves selected ALSA drain, not
/// that a person heard the speaker or that another Host has this route.
#[test]
#[ignore = "requires explicit ALSA card/device, installed eSpeak NG, and a real speaker"]
fn selected_installed_birth_speaks_one_current_face_clause() {
    let card = std::env::var("CONDUIT_SPOKEN_TEST_ALSA_CARD").unwrap();
    let device = std::env::var("CONDUIT_SPOKEN_TEST_ALSA_DEVICE").unwrap();
    let options = product(&["body", "speech-options", "--json"]);
    assert!(options.status.success());
    let options: Value = serde_json::from_slice(&options.stdout).unwrap();
    assert_eq!(options["schema"], "conduit.body/local-speech-options@1");
    assert!(options["speakers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|speaker| {
            speaker["card_id"] == card
                && speaker["device"].as_u64() == Some(device.parse::<u64>().unwrap())
        }));
    let [provider] = options["providers"].as_array().unwrap().as_slice() else {
        panic!("local proof requires exactly one verified eSpeak provider");
    };
    let state = std::env::temp_dir().join(format!(
        "conduit-selected-birth-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    let mut service = start_service(&state);
    let output = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
            "--speak",
            "--speaker-card",
            &card,
            "--speaker-device",
            &device,
            "--speech-executable",
            provider["executable"].as_str().unwrap(),
            "--speech-data",
            provider["data"].as_str().unwrap(),
            "--speech-engine",
            provider["engine"].as_str().unwrap(),
        ],
        b"stop\nfocus creche.name\n",
    );
    assert!(
        output.status.success(),
        "selected playback failed: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let receipts = stdout
        .lines()
        .filter_map(|line| {
            line.find('{')
                .and_then(|start| serde_json::from_str::<Value>(&line[start..]).ok())
        })
        .filter(|receipt| receipt["schema"] == "conduit.body/spoken-face-playback@1")
        .collect::<Vec<_>>();
    let played = receipts
        .iter()
        .find(|receipt| receipt["outcome"] == "Completed")
        .expect("selected speaker did not complete a Play");
    assert_eq!(played["speaker_lifecycle"], "StoppedClosed");
    assert_eq!(played["provider_sha256"], provider["provider_sha256"]);
    assert!(
        played["speaker_blocks_committed"].as_u64().unwrap() > 3_072,
        "selected speech must drain a real multi-block utterance beyond the old 3.48s cap"
    );
    assert!(played["speaker_frames_committed"].as_u64().unwrap() > 0);
    eprintln!("selected installed spoken playback receipt: {played}");
    assert!(stdout.contains("conduit.body/spoken-face-turn@1"));
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

fn seed_installation(state: &Path) {
    let executable = state.join("installed-conduit");
    fs::write(&executable, b"reviewed installed product image").unwrap();
    let bytes = fs::read(&executable).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(bytes));
    fs::write(
        state.join("installation.json"),
        serde_json::to_vec(&json!({
            "schema":"conduit.install/durable-host@1",
            "host_id":format!("host/installed/{}", state.file_name().unwrap().to_string_lossy()),
            "release_source_identity":"commit:installed-birth-proof",
            "release_bundle_sha256":digest,
            "product_executable":executable,
            "body_state":null,
            "joined_body_state":null
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(state.join("control.token"), [37_u8; 32]).unwrap();
}

fn start_service(state: &Path) -> Service {
    let service = Service(
        Command::new(env!("CARGO_BIN_EXE_conduit"))
            .args(["host", "service", "run", "--state-dir", path(state)])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !state.join("control.sock").exists() {
        assert!(
            Instant::now() < deadline,
            "installed service did not open control"
        );
        thread::sleep(Duration::from_millis(10));
    }
    service
}

fn stop_service(service: &mut Service) {
    service.0.kill().unwrap();
    service.0.wait().unwrap();
}

fn product(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(arguments)
        .output()
        .unwrap()
}

fn product_with_stdin(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn path(value: &Path) -> &str {
    value.to_str().unwrap()
}
