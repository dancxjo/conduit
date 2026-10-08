//! Fresh Birth and first browser Add. This producer remains deliberately partial.
//! Its raw product/browser receipts can seed a later continuous journey, but it
//! cannot issue the eight-chapter terminal or publication manifest.

use super::{regular, retain, sha};
use crate::cli::{GlobalOpts, TodoJourneyArgs};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_COMMAND_OUTPUT: usize = 512 * 1024;
const MAX_MEDIA: usize = 16 * 1024 * 1024;

struct Service(Child);

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn now() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis())
}

fn capture(
    root: &Path,
    label: &str,
    command: &mut Command,
    input: Option<&[u8]>,
) -> Result<Value, String> {
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start {label}: {error}"))?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or("Todo capture command has no stdin")?
            .write_all(input)
            .map_err(|error| format!("write {label} input: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for {label}: {error}"))?;
    if output.stdout.len() > MAX_COMMAND_OUTPUT || output.stderr.len() > MAX_COMMAND_OUTPUT {
        return Err(format!("{label} command output exceeds capture bound"));
    }
    let stdout = retain(root, &format!("{label}.stdout"), &output.stdout)?;
    let stderr = retain(root, &format!("{label}.stderr"), &output.stderr)?;
    if !output.status.success() {
        return Err(format!("{label} refused; raw stdout and stderr retained"));
    }
    Ok(json!({"stdout":stdout,"stderr":stderr,"exit_code":output.status.code()}))
}

fn json_output(root: &Path, label: &str, command: &mut Command) -> Result<Value, String> {
    capture(root, label, command, None)?;
    let bytes =
        fs::read(root.join(format!("{label}.stdout"))).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label} returned invalid JSON: {error}"))
}

fn media(root: &Path, relative: &str) -> Result<Value, String> {
    let path = root.join(relative);
    regular(&path)?;
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_MEDIA {
        return Err(format!("{relative} exceeds media bound or is empty"));
    }
    Ok(json!({"path":relative,"bytes":bytes.len(),"sha256":sha(&bytes)}))
}

fn selected_checkpoint(root: &Path, installation: &Value) -> Result<PathBuf, String> {
    let selected = installation["selected_todo_checkpoint"]["root"]
        .as_str()
        .ok_or("fresh Todo capture needs an installed selected checkpoint root")?;
    let path = Path::new(selected);
    if !path.is_absolute()
        || fs::symlink_metadata(path)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("selected checkpoint root must be an existing real directory".into());
    }
    let selected = fs::canonicalize(path).map_err(|error| error.to_string())?;
    if !selected.is_dir() || selected == root || root.starts_with(&selected) {
        return Err("selected checkpoint root must be a separate directory".into());
    }
    if fs::read_dir(&selected)
        .map_err(|error| error.to_string())?
        .next()
        .is_some()
    {
        return Err("fresh selected checkpoint root is not empty".into());
    }
    Ok(selected)
}

fn checkpoint_files(root: &Path) -> Result<Value, String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
            return Err("checkpoint directory contains a nonregular or oversized entry".into());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "checkpoint name is not UTF-8")?;
        entries.push((
            name,
            sha(&fs::read(entry.path()).map_err(|error| error.to_string())?),
        ));
        if entries.len() > 256 {
            return Err("checkpoint inventory exceeds bound".into());
        }
    }
    entries.sort();
    Ok(json!({"files":entries.len(),"entries":entries}))
}

fn birth_body_id(stdout: &[u8]) -> Result<String, String> {
    let lines = std::str::from_utf8(stdout).map_err(|error| error.to_string())?;
    let mut body = None;
    for line in lines.lines() {
        let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        if let Some(id) = value["biography"]["body_id"].as_str() {
            if body.as_deref().is_some_and(|prior| prior != id) {
                return Err("Birth command changed Body identity".into());
            }
            body = Some(id.to_owned());
        }
    }
    body.ok_or_else(|| "Birth command did not report a Body identity".into())
}

fn await_service(service: &mut Service, socket: &Path) -> Result<(), String> {
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
    opts: &GlobalOpts,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.terminal_script.is_some() {
        return Err("fresh Birth/Add capture does not accept a terminal script".into());
    }
    let source = fs::canonicalize(
        args.fresh_body_source
            .as_ref()
            .ok_or("missing Body source")?,
    )?;
    if source != fs::canonicalize(repository.join("plots/todo/checkpoint-once.conduit"))? {
        return Err("fresh Todo Birth must use the checked checkpoint-once Plot".into());
    }
    let handbook = fs::canonicalize(
        args.handbook_package
            .as_ref()
            .ok_or("missing --handbook-package")?,
    )?;
    let playwright = fs::canonicalize(
        args.pinned_playwright
            .as_ref()
            .ok_or("missing --pinned-playwright")?,
    )?;
    let item_text = args.first_item_text.as_deref().unwrap_or("Buy milk");
    if item_text.trim() != item_text || item_text.is_empty() || item_text.len() > 64 {
        return Err("first Todo item text must be 1..64 trimmed UTF-8 bytes".into());
    }
    let state = fs::canonicalize(&args.state_dir)?;
    let bin = fs::canonicalize(&args.conduit_bin)?;
    regular(&bin)?;
    let installation: Value = serde_json::from_slice(&fs::read(state.join("installation.json"))?)?;
    if installation["schema"] != "conduit.install/durable-host@1"
        || installation["body_state"].is_object()
        || installation["joined_body_state"].is_object()
        || state.join("body/biography.json").exists()
        || state.join("control.sock").exists()
        || installation["product_executable"] != bin.to_string_lossy().as_ref()
    {
        return Err(
            "fresh capture requires an idle installed, unowned Host with its exact executable"
                .into(),
        );
    }
    let selected = selected_checkpoint(&state, &installation)?;
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repository)
        .output()?;
    let commit = String::from_utf8(head.stdout)?.trim().to_owned();
    let clean = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repository)
        .output()?;
    if !head.status.success()
        || !clean.status.success()
        || !clean.stdout.is_empty()
        || installation["release_source_identity"] != commit
    {
        return Err(
            "fresh capture source and installed release are not the same clean commit".into(),
        );
    }
    if args.output.exists() {
        return Err("Todo capture output must be a new directory".into());
    }
    if opts.dry_run {
        println!(
            "would Birth one Todo Body at {} and capture one browser Add in {}",
            state.display(),
            args.output.display()
        );
        return Ok(());
    }
    fs::DirBuilder::new().mode(0o700).create(&args.output)?;
    let output = fs::canonicalize(&args.output)?;
    let started = now()?;
    let result = (|| -> Result<Value, String> {
        let mut birth = Command::new("timeout");
        birth
            .args(["-k", "5s", "30s"])
            .arg(&bin)
            .args(["body", "own"])
            .arg(&source)
            .args(["--state-dir"])
            .arg(&state)
            .args(["--name", "Groceries"]);
        let birth_command = capture(&output, "birth", &mut birth, None)?;
        let body_id = birth_body_id(
            &fs::read(output.join("birth.stdout")).map_err(|error| error.to_string())?,
        )?;
        let checkpoint_before = checkpoint_files(&selected)?;
        if checkpoint_before["files"] != 0 {
            return Err("Birth unexpectedly wrote a Todo checkpoint".into());
        }
        let service_log =
            fs::File::create(output.join("service.stderr")).map_err(|error| error.to_string())?;
        let service = Command::new(&bin)
            .args(["host", "service", "run", "--state-dir"])
            .arg(&state)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(service_log))
            .spawn()
            .map_err(|error| format!("start installed service: {error}"))?;
        let mut service = Service(service);
        await_service(&mut service, &state.join("control.sock"))?;
        let mut before = Command::new("timeout");
        before
            .args(["-k", "5s", "15s"])
            .arg(&bin)
            .args(["body", "face", "--state-dir"])
            .arg(&state)
            .arg("--json");
        let face_before = json_output(&output, "before-face", &mut before)?;
        if face_before["presentation"]["basis"]["body_id"] != body_id {
            return Err("fresh Owner Face names a different Body".into());
        }
        let mut browser = Command::new("timeout");
        browser
            .args(["-k", "5s", "180s", "node"])
            .arg(repository.join("proof/browser/todo-owner-browser.mjs"))
            .arg(repository)
            .arg(&bin)
            .arg(&state)
            .arg(&handbook)
            .arg(output.join("browser"))
            .arg(&playwright)
            .arg(item_text)
            .current_dir(repository);
        let browser_command = capture(&output, "browser-add", &mut browser, None)?;
        let browser_receipt: Value = serde_json::from_slice(
            &fs::read(output.join("browser/receipt.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if browser_receipt["schema"] != "conduit.proof/todo-owner-browser@1"
            || browser_receipt["source_relation"] != "exact-source"
            || browser_receipt["owner_source_commit"] != commit
            || browser_receipt["body_id"] != body_id
            || browser_receipt["item_text"] != item_text
            || browser_receipt["before"]["show_id"] == browser_receipt["after"]["show_id"]
        {
            return Err("browser Add receipt does not bind the fresh Body and exact source".into());
        }
        let before_png = media(&output, "browser/browser-before.png")?;
        let after_png = media(&output, "browser/browser-after.png")?;
        let browser_receipt_file = media(&output, "browser/receipt.json")?;
        let mut after = Command::new("timeout");
        after
            .args(["-k", "5s", "15s"])
            .arg(&bin)
            .args(["body", "face", "--state-dir"])
            .arg(&state)
            .arg("--json");
        let face_after = json_output(&output, "after-face", &mut after)?;
        if face_after["presentation"]["basis"]["body_id"] != body_id
            || !face_after["presentation"]["subjects"]
                .as_array()
                .is_some_and(|subjects| subjects.iter().any(|subject| subject["name"] == item_text))
        {
            return Err("installed Owner did not retain the browser-added Todo item".into());
        }
        let checkpoint_after = checkpoint_files(&selected)?;
        if checkpoint_after["files"].as_u64().unwrap_or(0) == 0 {
            return Err("browser Add did not publish a selected Todo checkpoint".into());
        }
        let mut terminal = Command::new("timeout");
        terminal
            .args(["-k", "5s", "30s"])
            .arg(&bin)
            .args(["body", "terminal", "--state-dir"])
            .arg(&state);
        let terminal_input = retain(&output, "terminal.input", b"quit\n")?;
        let terminal_command = capture(&output, "terminal", &mut terminal, Some(b"quit\n"))?;
        let terminal_text =
            fs::read(output.join("terminal.stdout")).map_err(|error| error.to_string())?;
        if !String::from_utf8_lossy(&terminal_text).contains(item_text) {
            return Err("terminal did not show the browser-added Todo item".into());
        }
        let mut terminal_face = Command::new("timeout");
        terminal_face
            .args(["-k", "5s", "15s"])
            .arg(&bin)
            .args(["body", "face", "--state-dir"])
            .arg(&state)
            .arg("--json");
        let face_after_terminal = json_output(&output, "after-terminal-face", &mut terminal_face)?;
        if face_after_terminal["presentation"]["basis"]["body_id"] != body_id
            || !face_after_terminal["presentation"]["subjects"]
                .as_array()
                .is_some_and(|subjects| subjects.iter().any(|subject| subject["name"] == item_text))
            || checkpoint_files(&selected)? != checkpoint_after
        {
            return Err("terminal read changed the Body or selected checkpoint".into());
        }
        Ok(json!({
            "body_id":body_id,
            "birth":{"command":birth_command,"face_id":face_before["presentation"]["identity"],
                "face_revision":face_before["presentation"]["revision"]},
            "add":{"command":browser_command,"receipt":browser_receipt_file,
                "before":browser_receipt["before"],"after":browser_receipt["after"],
                "owner_face_id":face_after["presentation"]["identity"],
                "owner_face_revision":face_after["presentation"]["revision"],
                "screenshots":[before_png,after_png]},
            "selected_checkpoint_root":selected,
            "checkpoint_before":checkpoint_before,"checkpoint_after":checkpoint_after,
            "terminal":{"command":terminal_command,"input":terminal_input,
                "face_id":face_after_terminal["presentation"]["identity"],
                "face_revision":face_after_terminal["presentation"]["revision"]},
        }))
    })();
    let finished = now()?;
    let record = json!({
        "schema":"conduit.todo-journey/partial-live-capture@1",
        "capture_entrance":"cargo xtask prove todo-journey",
        "chapter_scope":["birth","add","terminal-read"],"publication_ready":false,
        "capture_tool_commit":commit,"installed_product_source_commit":installation["release_source_identity"],
        "installed_executable_sha256":sha(&fs::read(&bin)?),
        "started_at_unix_ms":started,"finished_at_unix_ms":finished,
        "observation":result.as_ref().ok(),"error":result.as_ref().err(),
        "missing_for_publication":["remaining five continuous chapters", "Add queue and child Sign correlation",
            "native and QMP captures", "requested-detail same-Play speech", "new-Boot recovery",
            "complete producer events and cross-Mask action receipts"]
    });
    fs::write(
        output.join("partial-run.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    result.map_err(|error| {
        format!(
            "fresh Todo Birth/Add failed: {error}; retained at {}",
            output.display()
        )
    })?;
    println!(
        "Retained fresh Todo Birth, browser Add, and terminal read at {}. Publication remains incomplete.",
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn birth_output_must_retain_one_body_identity() {
        let same = b"{\"biography\":{\"body_id\":\"body/exact\"}}\n{\"biography\":{\"body_id\":\"body/exact\"}}\n";
        assert_eq!(birth_body_id(same).unwrap(), "body/exact");
        let changed = b"{\"biography\":{\"body_id\":\"body/first\"}}\n{\"biography\":{\"body_id\":\"body/other\"}}\n";
        assert!(birth_body_id(changed).is_err());
        assert!(birth_body_id(b"{\"status\":\"birth\"}\n").is_err());
    }

    #[test]
    fn fresh_checkpoint_selection_refuses_existing_state_and_symlink() {
        let nonce = now().unwrap();
        let fixture = std::env::temp_dir().join(format!(
            "todo-fresh-preflight-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&fixture).unwrap();
        let state = fixture.join("state");
        let selected = state.join("selected");
        fs::create_dir_all(&selected).unwrap();
        let install = json!({"selected_todo_checkpoint":{"root":selected}});
        assert_eq!(selected_checkpoint(&state, &install).unwrap(), selected);
        fs::write(selected.join("already.checkpoint"), b"retained").unwrap();
        assert!(selected_checkpoint(&state, &install).is_err());
        fs::remove_file(selected.join("already.checkpoint")).unwrap();
        let link = state.join("linked");
        symlink(&selected, &link).unwrap();
        assert!(
            selected_checkpoint(&state, &json!({"selected_todo_checkpoint":{"root":link}}))
                .is_err()
        );
        fs::remove_dir_all(&fixture).unwrap();
    }
}
