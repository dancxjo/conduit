//! Retain an actual installed Todo owner's terminal encounter for the journey.
//!
//! Terminal-only modes retain partial receipts. Fresh direct-speech mode also
//! captures the browser journey, selected WAV artifacts, recovery and refusal
//! before producing the complete local publication packet.

use crate::{
    cli::{GlobalOpts, TodoJourneyArgs},
    workspace::workspace_root,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_OUTPUT: usize = 512 * 1024;
const MAX_SCRIPT: usize = 4096;

mod fresh;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn regular(path: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err(format!("expected a regular file: {}", path.display()));
    }
    Ok(())
}

fn retain(root: &Path, name: &str, bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_OUTPUT {
        return Err(format!("{name} exceeds capture bound"));
    }
    fs::write(root.join(name), bytes).map_err(|e| e.to_string())?;
    Ok(json!({"path":name,"bytes":bytes.len(),"sha256":sha(bytes)}))
}

struct InstalledCapture<'a> {
    bin: &'a Path,
    root: &'a Path,
    state: &'a Path,
}

impl InstalledCapture<'_> {
    fn invoke(
        &self,
        steps: &mut Vec<Value>,
        label: &str,
        command: &[&str],
        after_state: &[&str],
        input: Option<&[u8]>,
    ) -> Result<(), String> {
        let mut child = Command::new(self.bin)
            .args(command)
            .arg("--state-dir")
            .arg(self.state)
            .args(after_state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("start {label}: {e}"))?;
        if let Some(bytes) = input {
            child
                .stdin
                .take()
                .ok_or("terminal stdin missing")?
                .write_all(bytes)
                .map_err(|e| format!("write {label} commands: {e}"))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("wait for {label}: {e}"))?;
        let stdout = retain(self.root, &format!("{label}.stdout"), &output.stdout)?;
        let stderr = retain(self.root, &format!("{label}.stderr"), &output.stderr)?;
        let receipt = json!({"command":command,"state_dir":self.state,
            "arguments_after_state_dir":after_state,"exit_code":output.status.code(),"stdout":stdout,"stderr":stderr});
        steps.push(receipt);
        if !output.status.success() {
            return Err(format!("{label} failed; raw output retained"));
        }
        Ok(())
    }
}

fn parse_capture(root: &Path, label: &str) -> Result<Value, String> {
    serde_json::from_slice(
        &fs::read(root.join(format!("{label}.stdout"))).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{label} did not return JSON: {e}"))
}

fn has_todo_list(face: &Value) -> bool {
    let presentation = &face["presentation"];
    presentation["subjects"].as_array().is_some_and(|subjects| {
        subjects
            .iter()
            .any(|subject| subject["identity"] == "todo/list")
    }) && presentation["properties"]
        .as_array()
        .is_some_and(|properties| {
            properties.iter().any(|property| {
                property["subject"] == "todo/list" && property["name"] == "todo-revision"
            })
        })
}

pub fn run(args: TodoJourneyArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let repository = workspace_root()?;
    if args.fresh_body_source.is_some() {
        return fresh::run(&args, opts, &repository);
    }
    if args.handbook_package.is_some()
        || args.pinned_playwright.is_some()
        || args.first_item_text.is_some()
        || args.cross_mask_actions
        || args.direct_speech
    {
        return Err("fresh Todo browser arguments require --fresh-body-source".into());
    }
    let state = fs::canonicalize(&args.state_dir)?;
    let bin = fs::canonicalize(&args.conduit_bin)?;
    regular(&bin)?;
    let install_path = state.join("installation.json");
    regular(&install_path)?;
    let install_bytes = fs::read(&install_path)?;
    if install_bytes.len() > 64 * 1024 {
        return Err("installation record exceeds bound".into());
    }
    let install: Value = serde_json::from_slice(&install_bytes)?;
    let bundle = install["release_bundle_sha256"]
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or("installed release bundle digest missing")?;
    if bundle.len() != 64 || !bundle.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("installed release bundle digest is invalid".into());
    }
    let recorded_executable = Path::new(
        install["product_executable"]
            .as_str()
            .ok_or("installed executable missing")?,
    );
    let filename = recorded_executable
        .file_name()
        .ok_or("installed executable name missing")?;
    let installed_bin = fs::canonicalize(state.join("releases").join(bundle).join(filename))?;
    if recorded_executable.is_absolute() {
        if fs::canonicalize(recorded_executable)? != installed_bin {
            return Err("recorded installed executable differs from release copy".into());
        }
    } else if !recorded_executable.ends_with(Path::new("releases").join(bundle).join(filename)) {
        return Err("relative installed executable lacks the selected release path".into());
    }
    if install["schema"] != "conduit.install/durable-host@1" || installed_bin != bin {
        return Err("capture executable differs from installed Host release".into());
    }
    let script = if let Some(path) = &args.terminal_script {
        regular(path)?;
        fs::read(path)?
    } else {
        b"quit\n".to_vec()
    };
    if script.is_empty()
        || script.len() > MAX_SCRIPT
        || std::str::from_utf8(&script).is_err()
        || !script.ends_with(b"\n")
        || std::str::from_utf8(&script)?.trim_end().lines().last() != Some("quit")
    {
        return Err("terminal script must be bounded UTF-8 ending commands with quit".into());
    }
    if args.output.exists() {
        return Err("Todo capture output must be a new directory".into());
    }
    if opts.dry_run {
        println!(
            "would observe installed owner {} with {} and retain a partial encounter in {}",
            state.display(),
            bin.display(),
            args.output.display()
        );
        return Ok(());
    }
    fs::create_dir(&args.output)?;
    let output = fs::canonicalize(&args.output)?;
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let capture_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()?;
    if !capture_commit.status.success() {
        return Err("cannot identify capture source commit".into());
    }
    let capture_commit = String::from_utf8(capture_commit.stdout)?.trim().to_owned();
    let capture = InstalledCapture {
        bin: &bin,
        root: &output,
        state: &state,
    };
    let mut steps = Vec::new();
    let result = (|| -> Result<Value, String> {
        capture.invoke(
            &mut steps,
            "before-status",
            &["host", "service", "status"],
            &["--json"],
            None,
        )?;
        capture.invoke(
            &mut steps,
            "before-face",
            &["body", "face"],
            &["--json"],
            None,
        )?;
        let before_status = parse_capture(&output, "before-status")?;
        let before_face = parse_capture(&output, "before-face")?;
        let body_id = before_face["presentation"]["basis"]["body_id"]
            .as_str()
            .ok_or("owner Face has no Body ID")?;
        if before_status["body_id"].as_str() != Some(body_id) {
            return Err("status and Face name different Bodies".into());
        }
        if !has_todo_list(&before_face) {
            return Err("installed owner Face has no Todo list contribution".into());
        }
        let script_receipt = retain(&output, "terminal.input", &script)?;
        capture.invoke(
            &mut steps,
            "terminal",
            &["body", "terminal"],
            &[],
            Some(&script),
        )?;
        capture.invoke(
            &mut steps,
            "after-status",
            &["host", "service", "status"],
            &["--json"],
            None,
        )?;
        capture.invoke(
            &mut steps,
            "after-face",
            &["body", "face"],
            &["--json"],
            None,
        )?;
        let after_face = parse_capture(&output, "after-face")?;
        if after_face["presentation"]["basis"]["body_id"].as_str() != Some(body_id) {
            return Err("terminal encounter changed Body identity".into());
        }
        if !has_todo_list(&after_face) {
            return Err("terminal encounter lost the Todo list contribution".into());
        }
        Ok(
            json!({"body_id":body_id,"before_face_revision":before_face["presentation"]["revision"],
            "after_face_revision":after_face["presentation"]["revision"],
            "host_id":before_face["advertisement"]["host_id"],"boot_id":before_face["advertisement"]["boot_id"],
            "terminal_input":script_receipt}),
        )
    })();
    let finished = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let record = json!({"schema":"conduit.todo-journey/partial-live-capture@1",
        "publication_ready":false,"capture_entrance":"cargo xtask prove todo-journey",
        "capture_tool_commit":capture_commit,"installed_product_source_commit":install["release_source_identity"],
        "installed_release_bundle_sha256":install["release_bundle_sha256"],
        "installed_executable_sha256":sha(&fs::read(&bin)?),"installation_record_sha256":sha(&install_bytes),
        "started_at_unix_ms":started,"finished_at_unix_ms":finished,
        "steps":steps,"observation":result.as_ref().ok(),"error":result.as_ref().err(),
        "missing_for_publication":["live browser action and Chromium screenshot","native screenshot",
            "QMP guest screenshot","two delivered-Play direct-speech WAV captures",
            "fresh-Boot recovery and complete eight-chapter event receipts"]});
    fs::write(
        output.join("partial-run.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    result.map_err(|error| {
        format!(
            "partial Todo capture failed: {error}; retained at {}",
            output.display()
        )
    })?;
    println!(
        "Retained live Todo terminal encounter at {}. Journey publication remains incomplete.",
        output.display()
    );
    Ok(())
}
