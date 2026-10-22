//! Bounded owner serial observations and exact guest readiness checks.
use super::*;
use std::{thread, time::Instant};

pub(super) fn wait_for_standby(
    serial_path: &std::path::Path,
    child: &mut Child,
    timeout: Duration,
    allow_read_only: bool,
) -> Result<(Value, Value, bool), ConduitosError> {
    let deadline = Instant::now() + timeout;
    loop {
        let serial = bounded_serial(serial_path)?;
        let part = records(&serial, GUEST_PART)?;
        let faces = records(&serial, OWNER_FACE)?;
        let routes = records(&serial, "CONDUIT_NATIVE_OWNER_ROUTE")?;
        let shown = faces.iter().find(|value| {
            value.get("status").and_then(Value::as_str) == Some("shown")
                && value.get("interactions_admitted") == Some(&Value::Bool(false))
                && value.get("local_show_available") == Some(&Value::Bool(true))
        });
        let route_available = routes.iter().any(|value| {
            value.get("status").and_then(Value::as_str) == Some("standby")
                && value.get("activation").and_then(Value::as_str) == Some("F5")
        });
        if serial.contains("CONDUIT_BOOT_STAGE front-door-ready")
            && (route_available || allow_read_only)
        {
            if let (Some(part), Some(shown)) = (part.last(), shown) {
                if part.get("membership_installed") == Some(&Value::Bool(true)) {
                    return Ok((part.clone(), shown.clone(), route_available));
                }
            }
        }
        wait_or_refuse(child, deadline, "native-owner-standby-not-ready")?;
    }
}

pub(super) fn clean_source(root: &std::path::Path) -> Result<bool, ConduitosError> {
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

pub(super) fn replace_qemu_option(
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

pub(super) fn wait_for_arrival(
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

pub(super) fn wait_for_action(
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
        if let Some(action) = actions.last() {
            if let Some((after, ack)) = refreshed_show_after_action(action, &faces) {
                return Ok((action.clone(), after, ack));
            }
        }
        wait_or_refuse(child, deadline, "native-owner-action-not-observed")?;
    }
}

pub(super) fn bounded_serial(path: &std::path::Path) -> Result<String, ConduitosError> {
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
            ));
        }
    }
    fs::read_to_string(path)
        .map_err(|error| ConduitosError::refusal("native-owner-serial-io", error.to_string()))
}

pub(super) fn records(serial: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
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

pub(super) fn wait_or_refuse(
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

pub(super) fn refusal(reason: &'static str) -> ConduitosError {
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
}
