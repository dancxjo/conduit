//! Deliberately corrupt only the checkpoint created by this fresh proof run.
//! Restore its exact bytes on every ordinary result before reencountering it.
use super::{recovery, sha, MAX_COMMAND_OUTPUT};
use crate::cli::TodoJourneyArgs;
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub(super) fn run(
    args: &TodoJourneyArgs,
    repository: &Path,
    output: &Path,
    bin: &Path,
    state: &Path,
    selected: &Path,
) -> Result<Value, String> {
    let root = output.join("checkpoint-refusal");
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    let execution: Value = serde_json::from_slice(
        &fs::read(state.join("body/owner-execution.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let before = &execution["last_execution"];
    if before["verified"] != true || before["read_terminal"] != "Completed" {
        return Err("checkpoint refusal proof requires the current verified read".into());
    }
    let version = before["selected_content"]["version"]
        .as_array()
        .ok_or("selected version is absent")?;
    if version.len() != 32 {
        return Err("selected version is not256bits".into());
    }
    let version: Vec<u8> = version
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or("invalid version byte")
        })
        .collect::<Result<_, _>>()?;
    let suffix = format!(
        ".{}.checkpoint",
        version
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    );
    let mut candidates = Vec::new();
    for entry in fs::read_dir(selected).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().ends_with(&suffix) {
            candidates.push(entry.path());
        }
    }
    if candidates.len() != 1 {
        return Err("expected one exact selected generation from this proof run".into());
    }
    let checkpoint = &candidates[0];
    let metadata = fs::symlink_metadata(checkpoint).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file() || metadata.len() > 1024 * 1024 {
        return Err("checkpoint is not a bounded regular file".into());
    }
    let original = fs::read(checkpoint).map_err(|e| e.to_string())?;
    if original.len() < 74 || original[41..73] != version {
        return Err("selected generation does not match immutable checkpoint header".into());
    }
    let result = (|| -> Result<Value, String> {
        let mut damaged = original.clone();
        *damaged.last_mut().ok_or("empty checkpoint")? ^= 1;
        fs::write(checkpoint, damaged).map_err(|e| e.to_string())?;
        let refused = Command::new("timeout")
            .args(["-k", "5s", "30s"])
            .arg(bin)
            .args(["host", "service", "run", "--state-dir"])
            .arg(state)
            .current_dir(repository)
            .output()
            .map_err(|e| e.to_string())?;
        if refused.stdout.len() > MAX_COMMAND_OUTPUT || refused.stderr.len() > MAX_COMMAND_OUTPUT {
            return Err("checkpoint refusal output exceeds bound".into());
        }
        fs::write(root.join("refused.stdout"), &refused.stdout).map_err(|e| e.to_string())?;
        fs::write(root.join("refused.stderr"), &refused.stderr).map_err(|e| e.to_string())?;
        if refused.status.success() || matches!(refused.status.code(), Some(124 | 137)) {
            return Err("corrupt checkpoint did not produce a finite service refusal".into());
        }
        let raw = fs::read(state.join("body/owner-execution.json")).map_err(|e| e.to_string())?;
        fs::write(root.join("refused-execution.json"), &raw).map_err(|e| e.to_string())?;
        let record: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let failed = &record["last_execution"];
        if failed["verified"] != false
            || failed["refusal"] != "todo-committed-corrupt"
            || !failed["restored_fore_sha256"].is_null()
            || failed["read_kernel_failure"]["detail"] != 2
            || failed["body_id"] != before["body_id"]
            || !String::from_utf8_lossy(&refused.stderr).contains("todo-committed-corrupt")
        {
            return Err(
                "corruption did not retain the exact visible refusal without committed Fore".into(),
            );
        }
        Ok(
            json!({"exit_code":refused.status.code(),"body_id":failed["body_id"],"refusal":failed["refusal"],"verified":false,"restored_fore_sha256":null}),
        )
    })();
    // This repair runs even when the refusal observation returned an error.
    fs::write(checkpoint, &original).map_err(|e| format!("exact checkpoint repair failed: {e}"))?;
    if fs::read(checkpoint).map_err(|e| e.to_string())? != original {
        return Err("checkpoint exact-byte repair differs".into());
    }
    let refused = result?;
    let recovered = recovery::run(args, repository, &root, bin, state, before)?;
    Ok(
        json!({"refused":refused,"exact_repair_sha256":sha(&original),"recovery":recovered,
        "limits":["Deliberate corruption of the checkpoint born by this private proof run", "CLI refusal retained; no corrupt browser Face or physical playback claim"]}),
    )
}
