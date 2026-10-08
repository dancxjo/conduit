//! A public zero-Body client keeps the one service-owned Body after Birth.

#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
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
    let offline_zero_status = host_service_status(&state);
    assert_eq!(offline_zero_status["presence"], "installed-offline");
    assert!(offline_zero_status["body_id"].is_null());
    let mut service = start_service(&state);
    let zero_body_status = host_service_status(&state);
    assert_eq!(
        zero_body_status["schema"],
        "conduit.install/durable-host-runtime@1"
    );
    assert!(zero_body_status["body_id"].is_null());

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

    let one_plot = b"focus creche.plot.0\nedit value false\nactivate\nreview\nfocus creche.birth\nactivate\nread all\nquit\n";
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
    assert!(spoken.contains("Review Birth choices"));
    assert!(spoken.contains("Starting Plots selected:"));
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
    let born_status = host_service_status(&state);
    assert_eq!(born_status["body_id"], body_id);
    for field in [
        "host_id",
        "boot_id",
        "offer_generation",
        "process_id",
        "release_bundle_sha256",
    ] {
        assert_eq!(born_status[field], zero_body_status[field], "{field}");
    }

    let face = product(&["body", "face", "--state-dir", path(&state), "--json"]);
    assert!(
        face.status.success(),
        "{}",
        String::from_utf8_lossy(&face.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&face.stdout).unwrap();
    assert_eq!(snapshot.as_object().unwrap().len(), 4);
    assert_eq!(snapshot["schema"], "conduit.body/local-face-snapshot@1");
    assert_eq!(
        snapshot["presentation_revision_decimal"],
        snapshot["presentation"]["revision"].to_string()
    );
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
    let offline_status = host_service_status(&state);
    assert_eq!(
        offline_status["schema"],
        "conduit.install/durable-host-status@1"
    );
    assert_eq!(offline_status["presence"], "installed-offline");
    assert_eq!(offline_status["body_id"], body_id);
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let recovered_status = host_service_status(&state);
    assert_eq!(recovered_status["body_id"], body_id);
    assert_eq!(recovered_status["host_id"], born_status["host_id"]);
    assert_ne!(recovered_status["boot_id"], born_status["boot_id"]);
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
fn retained_nonvisual_client_changes_the_owner_terminal_wardrobe() {
    let state = std::env::temp_dir().join(format!(
        "conduit-screen-free-wardrobe-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    let mut service = start_service(&state);
    let born = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        b"focus creche.plot.1\nedit value true\nactivate\nfocus creche.birth\nactivate\nquit\n",
    );
    assert!(
        born.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&born.stderr),
        String::from_utf8_lossy(&born.stdout)
    );
    let body_id = host_service_status(&state)["body_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A second foreground terminal is a genuine owner-selected provider. Its
    // open connection holds the route while the nonvisual client changes the
    // same service-owned wardrobe through authenticated local control.
    let mut terminal = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "body",
            "terminal",
            "--state-dir",
            path(&state),
            "--owner-show",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut terminal_input = terminal.stdin.take().unwrap();
    let mut terminal_output = terminal.stdout.take().unwrap();
    let attached = read_until_prompt(
        &mut terminal_output,
        b"browser selection awaits a complete carrier-Line Mask Plan.\n",
    );
    assert!(attached.contains("Owner terminal Show"), "{attached}");

    let changed = product_with_stdin(
        &["body", "screen-free", "--state-dir", path(&state)],
        b"wardrobe\nwardrobe doff terminal\nwardrobe wear terminal\nwardrobe prefer terminal\nwardrobe\nquit\n",
    );
    assert!(
        changed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&changed.stderr),
        String::from_utf8_lossy(&changed.stdout)
    );
    let readout = String::from_utf8(changed.stdout).unwrap();
    assert!(!readout.contains("Wardrobe refused"), "{readout}");
    for revision in 0..=3 {
        assert!(
            readout.contains(&format!("Owner wardrobe revision {revision}.")),
            "{readout}"
        );
    }
    assert!(readout.contains("terminal on Host"), "{readout}");
    assert!(readout.contains("Current Mask wardrobe"), "{readout}");
    assert!(
        readout.contains("No current Show is acknowledged"),
        "{readout}"
    );
    assert!(
        readout.contains("Current acknowledged Show: none"),
        "{readout}"
    );
    assert_eq!(host_service_status(&state)["body_id"], body_id);

    terminal_input.write_all(b"quit\n").unwrap();
    let status = terminal.wait().unwrap();
    let mut stderr = String::new();
    terminal
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "terminal detachment: {stderr}");
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn post_birth_refuses_stale_activation_and_controls_clock() {
    let state = std::env::temp_dir().join(format!(
        "conduit-screen-free-stale-action-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    let mut service = start_service(&state);
    let mut client = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    let mut output = client.stdout.take().unwrap();
    input
        .write_all(
            b"focus creche.plot.1\nedit value true\nactivate\nfocus creche.birth\nactivate\n",
        )
        .unwrap();
    let born = read_until_prompt(&mut output, b"body> ");
    assert!(born.contains("Continuing retained Body"));

    let before = local_face(&state);
    let action = before["presentation"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["intent"] == "conduit.intent/change-clock-interval@1")
        .unwrap()["identity"]
        .as_str()
        .unwrap()
        .to_owned();
    input
        .write_all(format!("focus {action}\n").as_bytes())
        .unwrap();
    let focused = read_until_prompt(&mut output, b"body> ");
    assert!(!focused.contains("Refused action:"), "{focused}");

    // Leave time to exercise active-state controls even on a busy CI host.
    let started = product(&[
        "body",
        "start",
        "--state-dir",
        path(&state),
        "--maximum-millis",
        "30000",
    ]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let after = local_face(&state);
    assert_ne!(before["presentation"], after["presentation"]);

    // Read-only recovery uses the newly projected Face in the same turn;
    // unlike Activate, it cannot accidentally act on the old Show.
    input.write_all(b"read all\n").unwrap();
    let refreshed = read_until_prompt(&mut output, b"body> ");
    assert!(refreshed.contains("Text Face revision="), "{refreshed}");
    assert!(
        !refreshed.contains("Refused the pending command"),
        "{refreshed}"
    );

    let lull_action = action_id(&after, "conduit.intent/lull-clock@1");
    input
        .write_all(format!("focus {lull_action}\n").as_bytes())
        .unwrap();
    let _focused_lull = read_until_prompt(&mut output, b"body> ");
    let lulled = product(&["body", "lull", "--state-dir", path(&state)]);
    assert!(
        lulled.status.success(),
        "{}",
        String::from_utf8_lossy(&lulled.stderr)
    );
    // Lull acknowledges the stop request before the worker publishes its terminal Face.
    let after_lull = wait_for_available_action(&state, "conduit.intent/start-clock@1");
    assert_ne!(after["presentation"], after_lull["presentation"]);

    input.write_all(b"activate\n").unwrap();
    let stale = read_until_prompt(&mut output, b"body> ");
    assert!(stale.contains("Refused the pending command"), "{stale}");
    assert!(!stale.contains("Owner action result:"), "{stale}");
    assert!(!stale.contains("Owner action refused:"), "{stale}");

    // A new service Boot may retain the Body, but the command entered while
    // the previous Boot's Show was current still needs a fresh decision.
    stop_service(&mut service);
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let rebooted = local_face(&state);
    assert_eq!(
        after_lull["presentation"]["basis"]["body_id"],
        rebooted["presentation"]["basis"]["body_id"]
    );
    assert_ne!(
        after["advertisement"]["boot_id"],
        rebooted["advertisement"]["boot_id"]
    );
    input.write_all(b"activate\n").unwrap();
    let stale_boot = read_until_prompt(&mut output, b"body> ");
    assert!(
        stale_boot.contains("Refused the pending command"),
        "{stale_boot}"
    );
    assert!(!stale_boot.contains("Owner action result:"), "{stale_boot}");
    assert!(
        !stale_boot.contains("Owner action refused:"),
        "{stale_boot}"
    );

    // Reorient after the new Boot, then use only the current Face's typed
    // controls for a real retained Play, lull, and fresh start.
    let start = action_id(&rebooted, "conduit.intent/start-clock@1");
    input
        .write_all(format!("focus {start}\nactivate\n").as_bytes())
        .unwrap();
    let focused_start = read_until_prompt(&mut output, b"body> ");
    assert!(
        !focused_start.contains("Refused action:"),
        "{focused_start}"
    );
    let started_from_face = read_until_prompt(&mut output, b"body> ");
    assert!(
        started_from_face.contains("Owner action result:"),
        "{started_from_face}"
    );
    // Start acknowledges a request; observe the published Play before using
    // its controls, then explicitly reorient the client to that current Show.
    let playing = wait_for_available_action(&state, "conduit.intent/lull-clock@1");
    input.write_all(b"refresh\n").unwrap();
    let refreshed = read_until_prompt(&mut output, b"body> ");
    assert!(!refreshed.contains("Owner action result:"), "{refreshed}");
    assert!(!action_available(&playing, "conduit.intent/start-clock@1"));
    assert!(action_available(&playing, "conduit.intent/lull-clock@1"));
    let lull = action_id(&playing, "conduit.intent/lull-clock@1");
    input
        .write_all(format!("focus {lull}\nactivate\n").as_bytes())
        .unwrap();
    let focused_lull = read_until_prompt(&mut output, b"body> ");
    assert!(!focused_lull.contains("Refused action:"), "{focused_lull}");
    let lulled = read_until_prompt(&mut output, b"body> ");
    assert!(lulled.contains("Owner action result:"), "{lulled}");
    // Lull is also requested asynchronously; do not select the next Start
    // against a snapshot taken before retirement has reached the owner.
    let current = wait_for_available_action(&state, "conduit.intent/start-clock@1");
    input.write_all(b"refresh\n").unwrap();
    let refreshed = read_until_prompt(&mut output, b"body> ");
    assert!(!refreshed.contains("Owner action result:"), "{refreshed}");
    assert!(action_available(&current, "conduit.intent/start-clock@1"));
    assert!(!action_available(&current, "conduit.intent/lull-clock@1"));
    let wake = action_id(&current, "conduit.intent/start-clock@1");
    input
        .write_all(format!("focus {wake}\nactivate\n").as_bytes())
        .unwrap();
    let focused_wake = read_until_prompt(&mut output, b"body> ");
    assert!(!focused_wake.contains("Refused action:"), "{focused_wake}");
    let woke = read_until_prompt(&mut output, b"body> ");
    assert!(woke.contains("Owner action result:"), "{woke}");
    let resumed = wait_for_available_action(&state, "conduit.intent/lull-clock@1");
    assert!(!action_available(&resumed, "conduit.intent/start-clock@1"));
    assert!(action_available(&resumed, "conduit.intent/lull-clock@1"));
    input.write_all(b"read all\n").unwrap();
    let reoriented = read_until_prompt(&mut output, b"body> ");
    assert!(reoriented.contains("Text Face revision="), "{reoriented}");
    assert!(reoriented.contains("ticker pace"), "{reoriented}");

    input.write_all(b"quit\n").unwrap();
    assert!(client.wait().unwrap().success());
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn retained_screen_free_entrance_reopens_same_body_and_refuses_stale_boot() {
    let state = std::env::temp_dir().join(format!(
        "conduit-screen-free-return-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&state).unwrap();
    seed_installation(&state);
    let mut service = start_service(&state);
    let no_body = product_with_stdin(
        &["body", "screen-free", "--state-dir", path(&state)],
        b"quit\n",
    );
    assert!(!no_body.status.success());
    assert!(!state.join("body/biography.json").exists());
    let invalid_birth = product(&[
        "body",
        "birth",
        "--state-dir",
        path(&state),
        "--speak",
        "--speaker-card",
        "sofhdadsp",
        "--speaker-device",
        "0",
        "--speech-executable",
        "/usr/bin/espeak-ng",
        "--speech-data",
        "/usr/lib/espeak-ng-data",
        "--speech-engine",
        "/usr/lib/libespeak-ng.so.1",
        "--speech-language-coverage",
        "/unused/explicit-language-coverage.json",
    ]);
    assert!(!invalid_birth.status.success());
    assert!(String::from_utf8_lossy(&invalid_birth.stderr)
        .contains("--speak requires --screen-free for Birth"));

    let born = product_with_stdin(
        &[
            "body",
            "birth",
            "--screen-free",
            "--state-dir",
            path(&state),
        ],
        b"focus creche.plot.1\nedit value true\nactivate\nfocus creche.birth\nactivate\nquit\n",
    );
    assert!(
        born.status.success(),
        "{}",
        String::from_utf8_lossy(&born.stderr)
    );
    let before = local_face(&state);
    let body_id = before["presentation"]["basis"]["body_id"].as_str().unwrap();
    let start = action_id(&before, "conduit.intent/start-clock@1");
    let reopened = product_with_stdin(
        &["body", "screen-free", "--state-dir", path(&state)],
        format!("read all\nfocus {start}\nactivate\nquit\n").as_bytes(),
    );
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    let readout = String::from_utf8(reopened.stdout).unwrap();
    assert!(readout.contains(&format!("Continuing retained Body {body_id}")));
    assert!(readout.contains("Text Face revision="));
    assert!(readout.contains("ticker pace"));
    assert!(readout.contains("Owner action result:"));
    assert!(!readout.contains("Body retained by this installed Host:"));
    assert!(action_available(
        &local_face(&state),
        "conduit.intent/lull-clock@1"
    ));

    stop_service(&mut service);
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let rebooted = local_face(&state);
    assert_eq!(rebooted["presentation"]["basis"]["body_id"], body_id);
    assert_ne!(
        rebooted["advertisement"]["boot_id"],
        before["advertisement"]["boot_id"]
    );
    let lull = action_id(&rebooted, "conduit.intent/lull-clock@1");
    let mut client = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(["body", "screen-free", "--state-dir", path(&state)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    let mut output = client.stdout.take().unwrap();
    let arrival = read_until_prompt(&mut output, b"body> ");
    assert!(arrival.contains(&format!("Continuing retained Body {body_id}")));
    input
        .write_all(format!("focus {lull}\n").as_bytes())
        .unwrap();
    let _focused = read_until_prompt(&mut output, b"body> ");
    stop_service(&mut service);
    fs::remove_file(state.join("control.sock")).unwrap();
    service = start_service(&state);
    let latest = local_face(&state);
    assert_eq!(latest["presentation"]["basis"]["body_id"], body_id);
    assert_ne!(
        latest["advertisement"]["boot_id"],
        rebooted["advertisement"]["boot_id"]
    );
    input.write_all(b"activate\n").unwrap();
    let stale = read_until_prompt(&mut output, b"body> ");
    assert!(stale.contains("Refused the pending command"), "{stale}");
    assert!(!stale.contains("Owner action result:"), "{stale}");
    input.write_all(b"quit\n").unwrap();
    assert!(client.wait().unwrap().success());
    stop_service(&mut service);
    fs::remove_dir_all(state).unwrap();
}

fn local_face(state: &Path) -> Value {
    let response = product(&["body", "face", "--state-dir", path(state), "--json"]);
    assert!(
        response.status.success(),
        "{}",
        String::from_utf8_lossy(&response.stderr)
    );
    serde_json::from_slice(&response.stdout).unwrap()
}

fn action_id(face: &Value, intent: &str) -> String {
    face["presentation"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["intent"] == intent)
        .unwrap_or_else(|| panic!("missing current action {intent}"))["identity"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn action_available(face: &Value, intent: &str) -> bool {
    face["presentation"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["intent"] == intent)
        .unwrap_or_else(|| panic!("missing current action {intent}"))["availability"]
        == "Available"
}

fn wait_for_available_action(state: &Path, intent: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let face = local_face(state);
        if action_available(&face, intent) {
            return face;
        }
        assert!(
            Instant::now() < deadline,
            "owner did not publish {intent}: {face}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn read_until_prompt(input: &mut impl Read, prompt: &[u8]) -> String {
    let mut captured = Vec::new();
    let mut byte = [0];
    while !captured.ends_with(prompt) {
        assert_eq!(
            input.read(&mut byte).unwrap(),
            1,
            "client ended before prompt"
        );
        captured.push(byte[0]);
        assert!(captured.len() < 2_000_000, "unbounded screen-free readout");
    }
    String::from_utf8(captured).unwrap()
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
    let mut service = start_service(&state);
    // The journal can be written, but publication of its biography cannot
    // replace a directory. Inject this after startup so archive validation does
    // not refuse the Host before Birth reaches its commit decision.
    fs::create_dir_all(state.join("body/biography.json")).unwrap();
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
    let coverage = std::env::var("CONDUIT_SPOKEN_TEST_LANGUAGE_COVERAGE").unwrap();
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
            "--speech-language-coverage",
            &coverage,
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

fn host_service_status(state: &Path) -> Value {
    let output = product(&[
        "host",
        "service",
        "status",
        "--state-dir",
        path(state),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "host service status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
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
