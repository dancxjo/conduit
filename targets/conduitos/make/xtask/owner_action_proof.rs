//! Real QMP input against one exact private owner-provisioned product image.
//! The installed owner and its bounded invitation are prepared independently.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};

use crate::cli::GlobalOpts;

use super::{
    demo, journey_input, owner_boot, profile::Paths, qmp, qmp_display, report::git_head,
    ConduitosArch, ConduitosError, LiveOwnerActionProofArgs,
};

#[path = "owner_action_coordination.rs"]
mod coordination;
use coordination::{wait_for_resume, write_checkpoint};

const MAX_SERIAL_BYTES: u64 = 2 * 1024 * 1024;
const OWNER_FACE: &str = "CONDUIT_NATIVE_OWNER_FACE ";
const GUEST_PART: &str = "CONDUIT_NATIVE_GUEST_PART ";
const OWNER_ACTION: &str = "CONDUIT_NATIVE_OWNER_ACTION ";

pub(super) fn execute(
    args: &LiveOwnerActionProofArgs,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal("native-owner-proof-requires-live-input"));
    }
    let directory = fs::canonicalize(&args.output_dir).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-directory", error.to_string())
    })?;
    let metadata = fs::metadata(&directory).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-directory", error.to_string())
    })?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o077 != 0
        || fs::read_dir(&directory)
            .map_err(|error| {
                ConduitosError::refusal("native-owner-proof-directory", error.to_string())
            })?
            .next()
            .is_some()
    {
        return Err(refusal(
            "native-owner-proof-directory-must-be-empty-private",
        ));
    }
    let route = owner_boot::prepare(&args.spore, &args.candidate_id, args.owner_forward)?;
    let root = Paths::new(ConduitosArch::X86_64)?.root;
    let source = git_head(&root)?;
    if source != route.source_identity || !clean_source(&root)? {
        return Err(refusal("native-owner-proof-source-not-exact"));
    }
    let spore = fs::canonicalize(&args.spore)
        .map_err(|error| ConduitosError::refusal("native-owner-proof-spore", error.to_string()))?;
    let spore_str = spore
        .to_str()
        .ok_or_else(|| refusal("native-owner-proof-spore-utf8"))?;
    let serial_path = directory.join("guest.serial.log");
    let qmp_path = directory.join("qmp.sock");
    let qmp_arg = owner_boot::qmp_arg(&qmp_path)?;
    let serial_arg = format!("file:{}", serial_path.display());
    let mut qemu_args: Vec<String> =
        demo::qemu_args(spore_str, Some(&route.netdev), Some(&qmp_arg))
            .into_iter()
            .map(str::to_owned)
            .collect();
    replace_qemu_option(&mut qemu_args, "-display", "none")?;
    replace_qemu_option(&mut qemu_args, "-serial", &serial_arg)?;
    let stderr = fs::File::create(directory.join("qemu.stderr")).map_err(|error| {
        ConduitosError::refusal("native-owner-proof-artifact", error.to_string())
    })?;
    let mut child = Command::new("qemu-system-x86_64")
        .args(&qemu_args)
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-qemu", error.to_string()))?;
    let result = prove(
        &directory,
        &serial_path,
        &qmp_path,
        &mut child,
        &route,
        &qemu_args,
        args.coordinate,
    );
    let _ = child.kill();
    let _ = child.wait();
    let receipt = result?;
    let path = directory.join("owner-action-proof.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ConduitosError::refusal("native-owner-proof-encode", error.to_string())
        })?,
    )
    .map_err(|error| ConduitosError::refusal("native-owner-proof-artifact", error.to_string()))?;
    if opts.json {
        println!("{receipt}");
    } else if !opts.quiet {
        println!("Native owner action proof: {}", path.display());
    }
    Ok(())
}

fn prove(
    directory: &std::path::Path,
    serial_path: &std::path::Path,
    qmp_path: &std::path::Path,
    child: &mut Child,
    route: &owner_boot::PreparedOwnerBoot,
    qemu_args: &[String],
    coordinate: bool,
) -> Result<Value, ConduitosError> {
    let (mut qmp, mut reader) =
        qmp::connect_traced(qmp_path, child, Some(&directory.join("qmp.jsonl")))?;
    let (part, before, before_ack) =
        wait_for_arrival(serial_path, child, Duration::from_secs(120))?;
    let (before_image, health) =
        qmp_display::capture(&mut qmp, &mut reader, directory, "owner-before")?;
    if let Some(error) = health {
        return Err(error);
    }
    if coordinate {
        write_checkpoint(
            directory,
            "native-arrived.json",
            &json!({
                "schema":"conduit.conduitos/native-owner-coordination@1",
                "stage":"arrived",
                "guest_part":part,
                "face":before,
                "show_ack":before_ack,
            }),
        )?;
        wait_for_resume(directory, "resume-native-action", child)?;
    }
    // Keyboard traffic is the actual native Mask Fore: focus the only
    // available clock interval action, replace it with 500, then submit.
    for key in ["tab", "5", "0", "0", "ret"] {
        journey_input::key_pair(&mut qmp, &mut reader, key, "native-owner-clock-input")?;
    }
    let (action, after, after_ack) = wait_for_action(serial_path, child, Duration::from_secs(30))?;
    let (after_image, health) =
        qmp_display::capture(&mut qmp, &mut reader, directory, "owner-after")?;
    if let Some(error) = health {
        return Err(error);
    }
    validate_success(&before, &before_ack, &action, &after, &after_ack)?;
    if coordinate {
        write_checkpoint(
            directory,
            "native-action.json",
            &json!({
                "schema":"conduit.conduitos/native-owner-coordination@1",
                "stage":"action",
                "action":action,
                "face":after,
                "show_ack":after_ack,
            }),
        )?;
        wait_for_resume(directory, "resume-native-finish", child)?;
    }
    if child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-qemu", error.to_string()))?
        .is_some()
    {
        return Err(refusal("native-owner-guest-not-live-at-capture"));
    }
    Ok(json!({
        "schema":"conduit.conduitos/native-owner-action-proof@1",
        "proof_class":"live-local-qmp-installed-owner",
        "source_commit":route.source_identity,
        "spore_build_id":route.build_id,
        "spore_sha256":route.artifact_sha256,
        "candidate_id":route.candidate_id,
        "reachability":route.reachability,
        "qemu_argv":qemu_args,
        "guest_part":part,
        "face_before":before,
        "show_ack_before":before_ack,
        "action":action,
        "face_after":after,
        "show_ack_after":after_ack,
        "screenshots":[before_image,after_image],
        "qemu_alive_at_capture":true,
        "coordinated":coordinate,
    }))
}

fn validate_success(
    before: &Value,
    before_ack: &Value,
    action: &Value,
    after: &Value,
    after_ack: &Value,
) -> Result<(), ConduitosError> {
    if action.get("status").and_then(Value::as_str) != Some("accepted")
        || action.get("face_refreshed").and_then(Value::as_bool) != Some(true)
        || action.get("requested_interval_ms").and_then(Value::as_u64) != Some(500)
        || !action
            .get("action_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("body/action/change-clock-interval/"))
        || after.get("status").and_then(Value::as_str) != Some("shown")
        || before_ack.get("status").and_then(Value::as_str) != Some("acknowledged")
        || after_ack.get("status").and_then(Value::as_str) != Some("acknowledged")
        || before_ack.get("show_id") != before.get("show_id")
        || after_ack.get("show_id") != after.get("show_id")
        || after.get("local_show_available").and_then(Value::as_bool) != Some(true)
        || after
            .get("owner_show_acknowledged")
            .and_then(Value::as_bool)
            != Some(false)
        || action.get("prior_show_id") != before.get("show_id")
        || action.get("face_id") != before.get("face_id")
        || action.get("face_revision") != before.get("face_revision")
        || before.get("show_id") == after.get("show_id")
        || before.get("face_id") == after.get("face_id")
    {
        return Err(refusal("native-owner-action-not-accepted-and-refreshed"));
    }
    Ok(())
}

fn clean_source(root: &std::path::Path) -> Result<bool, ConduitosError> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-source", error.to_string()))?;
    if !output.status.success() {
        return Err(refusal("native-owner-proof-source-unavailable"));
    }
    Ok(output.stdout.is_empty())
}

fn replace_qemu_option(
    qemu_args: &mut [String],
    option: &'static str,
    value: &str,
) -> Result<(), ConduitosError> {
    let position = qemu_args
        .iter()
        .position(|argument| argument == option)
        .ok_or_else(|| refusal("native-owner-proof-qemu-option-missing"))?;
    let slot = qemu_args
        .get_mut(position + 1)
        .ok_or_else(|| refusal("native-owner-proof-qemu-option-missing"))?;
    *slot = value.to_owned();
    Ok(())
}

fn wait_for_arrival(
    serial_path: &std::path::Path,
    child: &mut Child,
    timeout: Duration,
) -> Result<(Value, Value, Value), ConduitosError> {
    let deadline = Instant::now() + timeout;
    loop {
        let serial = bounded_serial(serial_path)?;
        let part = records(&serial, GUEST_PART)?;
        let face = records(&serial, OWNER_FACE)?;
        let shown = face
            .iter()
            .rfind(|value| value.get("status").and_then(Value::as_str) == Some("shown"));
        let acknowledged = face
            .iter()
            .rfind(|value| value.get("status").and_then(Value::as_str) == Some("acknowledged"));
        if serial.contains("CONDUIT_BOOT_STAGE front-door-ready")
            && part
                .last()
                .is_some_and(|value| value.get("membership_installed") == Some(&Value::Bool(true)))
            && shown.is_some_and(|value| {
                value.get("local_show_available") == Some(&Value::Bool(true))
                    && value.get("owner_show_acknowledged") == Some(&Value::Bool(false))
                    && value.get("interactions_admitted") == Some(&Value::Bool(true))
                    && value.get("continuing_owner_route") == Some(&Value::Bool(true))
            })
            && acknowledged
                .is_some_and(|value| value.get("show_id") == shown.unwrap().get("show_id"))
        {
            return Ok((
                part.last().unwrap().clone(),
                shown.unwrap().clone(),
                acknowledged.unwrap().clone(),
            ));
        }
        wait_or_refuse(child, deadline, "native-owner-face-not-ready")?;
    }
}

fn wait_for_action(
    serial_path: &std::path::Path,
    child: &mut Child,
    timeout: Duration,
) -> Result<(Value, Value, Value), ConduitosError> {
    let deadline = Instant::now() + timeout;
    loop {
        let serial = bounded_serial(serial_path)?;
        let actions = records(&serial, OWNER_ACTION)?;
        let faces = records(&serial, OWNER_FACE)?;
        if let Some(action) = actions.last() {
            if action.get("status").and_then(Value::as_str) != Some("accepted") {
                return Err(ConduitosError::refusal(
                    "native-owner-action-not-accepted",
                    action
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("owner returned no finite refusal code"),
                ));
            }
        }
        let shown: Vec<&Value> = faces
            .iter()
            .filter(|value| value.get("status").and_then(Value::as_str) == Some("shown"))
            .collect();
        let acknowledged: Vec<&Value> = faces
            .iter()
            .filter(|value| value.get("status").and_then(Value::as_str) == Some("acknowledged"))
            .collect();
        if let (Some(action), Some(after), Some(ack)) =
            (actions.last(), shown.get(1), acknowledged.get(1))
        {
            if ack.get("show_id") == after.get("show_id") {
                return Ok((action.clone(), (*after).clone(), (*ack).clone()));
            }
        }
        wait_or_refuse(child, deadline, "native-owner-action-not-observed")?;
    }
}

fn bounded_serial(path: &std::path::Path) -> Result<String, ConduitosError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > MAX_SERIAL_BYTES => {
            return Err(refusal("native-owner-serial-pressure"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => {
            return Err(ConduitosError::refusal(
                "native-owner-serial-io",
                error.to_string(),
            ))
        }
    }
    fs::read_to_string(path)
        .map_err(|error| ConduitosError::refusal("native-owner-serial-io", error.to_string()))
}

fn records(serial: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    let mut result = Vec::new();
    // QEMU appends this file while the proof polls it. The final unterminated
    // line is still in flight; only a newline makes a serial record complete.
    let complete = serial
        .rsplit_once('\n')
        .map_or("", |(complete, _)| complete);
    for line in complete
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
    {
        if line.len() > 1024 || result.len() == 8 {
            return Err(refusal("native-owner-record-pressure"));
        }
        result.push(serde_json::from_str(line).map_err(|error| {
            ConduitosError::refusal("native-owner-record-invalid", error.to_string())
        })?);
    }
    Ok(result)
}

fn wait_or_refuse(
    child: &mut Child,
    deadline: Instant,
    reason: &'static str,
) -> Result<(), ConduitosError> {
    if child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("native-owner-proof-qemu", error.to_string()))?
        .is_some()
        || Instant::now() >= deadline
    {
        return Err(refusal(reason));
    }
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

fn refusal(reason: &'static str) -> ConduitosError {
    ConduitosError::refusal(
        reason,
        "the live owner/guest action did not satisfy the exact proof contract",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_records_remain_exact_and_bounded() {
        let line = format!("{OWNER_ACTION}{{\"status\":\"accepted\"}}\n");
        let parsed = records(&line, OWNER_ACTION).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["status"], "accepted");
        assert!(records(&line.repeat(9), OWNER_ACTION).is_err());
        assert!(records(&format!("{OWNER_ACTION}not-json\n"), OWNER_ACTION).is_err());
        assert!(records(
            &format!("{line}{OWNER_ACTION}{{\"status\":\"pend"),
            OWNER_ACTION
        )
        .is_ok_and(|records| records.len() == 1));
    }

    #[test]
    fn qemu_option_replacement_survives_singleton_flags() {
        let mut args = ["-no-reboot", "-display", "gtk", "-serial", "stdio"].map(str::to_owned);
        replace_qemu_option(&mut args, "-display", "none").unwrap();
        replace_qemu_option(&mut args, "-serial", "file:/tmp/guest.serial.log").unwrap();
        assert_eq!(args[2], "none");
        assert_eq!(args[4], "file:/tmp/guest.serial.log");
        assert!(replace_qemu_option(&mut args, "-qmp", "bad").is_err());
    }

    #[test]
    fn action_proof_rejects_stale_basis_and_unaccepted_return() {
        let before =
            json!({"status":"shown","show_id":"show/one","face_id":"face/one","face_revision":1});
        let accepted = json!({
            "status":"accepted","face_refreshed":true,"requested_interval_ms":500,
            "action_id":"body/action/change-clock-interval/1",
            "prior_show_id":"show/one","face_id":"face/one","face_revision":1,
        });
        let after = json!({"status":"shown","local_show_available":true,"owner_show_acknowledged":false,"show_id":"show/two","face_id":"face/two"});
        let before_ack = json!({"status":"acknowledged","show_id":"show/one"});
        let after_ack = json!({"status":"acknowledged","show_id":"show/two"});
        assert!(validate_success(&before, &before_ack, &accepted, &after, &after_ack).is_ok());
        let mut invented_owner_ack = after.clone();
        invented_owner_ack["owner_show_acknowledged"] = json!(true);
        assert!(validate_success(
            &before,
            &before_ack,
            &accepted,
            &invented_owner_ack,
            &after_ack
        )
        .is_err());
        let mut stale = accepted.clone();
        stale["prior_show_id"] = json!("show/older");
        assert!(validate_success(&before, &before_ack, &stale, &after, &after_ack).is_err());
        let mut refused = accepted.clone();
        refused["status"] = json!("refused");
        assert!(validate_success(&before, &before_ack, &refused, &after, &after_ack).is_err());
        let mut wrong_value = accepted;
        wrong_value["requested_interval_ms"] = json!(250);
        assert!(validate_success(&before, &before_ack, &wrong_value, &after, &after_ack).is_err());
    }
}
