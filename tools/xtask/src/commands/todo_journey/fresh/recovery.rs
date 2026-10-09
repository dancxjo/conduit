//! Reencounter the same committed Todo through a fresh Owner Boot and Browser Host.
use super::{capture, json_output, now, Service};
use crate::cli::TodoJourneyArgs;
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn socket_stamp(path: &Path) -> Option<(u64, u64, i64, i64)> {
    fs::metadata(path)
        .ok()
        .map(|s| (s.dev(), s.ino(), s.ctime(), s.ctime_nsec()))
}

pub(super) fn await_initial_service(service: &mut Service, socket: &Path) -> Result<(), String> {
    for _ in 0..100 {
        if socket.exists() {
            return Ok(());
        }
        if service
            .0
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("installed Owner service exited before control socket".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("installed Owner service did not open control socket".into())
}

pub(super) fn run(
    args: &TodoJourneyArgs,
    repository: &Path,
    output: &Path,
    bin: &Path,
    state: &Path,
    before: &Value,
) -> Result<Value, String> {
    if before["schema"] != "conduit.todo/verified-read-receipt@1"
        || before["verified"] != true
        || before["read_terminal"] != "Completed"
    {
        return Err(
            "recovery requires the current verified checkpoint read before restarting the Owner"
                .into(),
        );
    }
    let root = output.join("recovery");
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    let socket = state.join("control.sock");
    let previous = socket_stamp(&socket);
    let log = fs::File::create(root.join("service.stderr")).map_err(|e| e.to_string())?;
    let child = Command::new(bin)
        .args(["host", "service", "run", "--state-dir"])
        .arg(state)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut service = Service(child);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if service.0.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("recovery service exited before current control socket".into());
        }
        let current = socket_stamp(&socket);
        if current.is_some() && current != previous {
            break;
        }
        if Instant::now() >= deadline {
            return Err("recovery observation expired; lifecycle action was not retried".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    let mut command = Command::new("timeout");
    command
        .args(["-k", "5s", "15s"])
        .arg(bin)
        .args(["body", "face", "--state-dir"])
        .arg(state)
        .arg("--json");
    let face = json_output(&root, "face", &mut command)?;
    let raw = fs::read(state.join("body/owner-execution.json")).map_err(|e| e.to_string())?;
    fs::write(root.join("owner-execution.json"), &raw).map_err(|e| e.to_string())?;
    let record: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let after = &record["last_execution"];
    if after["schema"] != "conduit.todo/verified-read-receipt@1"
        || after["verified"] != true
        || after["read_terminal"] != "Completed"
        || !after["refusal"].is_null()
        || after["body_id"] != before["body_id"]
        || face["presentation"]["basis"]["body_id"] != before["body_id"]
        || after["restored_fore_sha256"] != before["restored_fore_sha256"]
        || after["selected_content"]["version"] != before["selected_content"]["version"]
        || after["selected_residence"]["owner_host"] != before["selected_residence"]["owner_host"]
        || after["selected_residence"]["owner_boot"] == before["selected_residence"]["owner_boot"]
        || after["read_play"]["active_play_id"] == before["read_play"]["active_play_id"]
        || after["read_terminal_sign"]["active_play_id"] != after["read_play"]["active_play_id"]
    {
        return Err("fresh recovery did not verify the same Body/state with a distinct Boot and correlated read Play".into());
    }
    let mut browser = Command::new("timeout");
    browser
        .args(["-k", "5s", "90s", "node"])
        .arg(repository.join("proof/browser/todo-owner-reencounter.mjs"))
        .arg(repository)
        .arg(bin)
        .arg(state)
        .arg(args.handbook_package.as_ref().ok_or("missing Handbook")?)
        .arg(root.join("browser"))
        .arg(
            args.pinned_playwright
                .as_ref()
                .ok_or("missing Playwright")?,
        )
        .current_dir(repository);
    capture(&root, "browser", &mut browser, None)?;
    let encounter: Value = serde_json::from_slice(
        &fs::read(root.join("browser/receipt.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if encounter["body_id"] != after["body_id"]
        || encounter["owner_boot_id"] != after["selected_residence"]["owner_boot"]
        || encounter["new_birth"] != false
        || encounter["todo_actions_executed"] != 0
        || encounter["source_relation"] != "exact-source"
    {
        return Err("browser recovery receipt does not bind the exact verified fresh Owner".into());
    }
    Ok(
        json!({"body_id":after["body_id"], "previous_boot_id":before["selected_residence"]["owner_boot"],
        "new_boot_id":after["selected_residence"]["owner_boot"], "pre_lull_state_sha256":before["restored_fore_sha256"],
        "recovered_state_sha256":after["restored_fore_sha256"], "read_play":after["read_play"],
        "terminal_sign":after["read_terminal_sign"], "browser":encounter, "observed_at_unix_ms":now()?,
        "new_birth":false, "limits":["Distinct Browser Host on the same physical machine", "Finite checkpoint Plays lull themselves; no long-running cancellation claim"]}),
    )
}
