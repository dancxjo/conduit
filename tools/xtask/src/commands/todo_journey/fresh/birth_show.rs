//! Observe the newly born Body through an actual terminal Mask before any Todo action.
use super::{capture, now, retain};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub(super) fn run(output: &Path, bin: &Path, state: &Path, body: &str) -> Result<Value, String> {
    let keys = b"wardrobe wear\nwardrobe prefer\nshow\nevidence\nquit\n";
    let input = retain(output, "birth-show.input", keys)?;
    let mut command = Command::new("timeout");
    command
        .args(["-k", "5s", "30s"])
        .arg(bin)
        .args(["body", "terminal", "--owner-show", "--state-dir"])
        .arg(state);
    let receipt = capture(output, "birth-show", &mut command, Some(keys))?;
    let observed = now()?;
    let text = fs::read_to_string(output.join("birth-show.stdout")).map_err(|e| e.to_string())?;
    let records: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Owner terminal evidence "))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Birth Show evidence JSON: {e}"))?;
    if records.len() != 1 {
        return Err("Birth Show needs exactly one requested terminal evidence record".into());
    }
    let evidence = &records[0];
    if evidence["schema"] != "conduit.body/terminal-show-evidence@1"
        || evidence["body_id"] != body
        || evidence["show_state"] != "available"
        || evidence["owner_selected_show_current"] != true
        || !text.contains(
            evidence["show_id"]
                .as_str()
                .ok_or("Birth Show lacks identity")?,
        )
    {
        return Err(
            "Birth terminal did not acknowledge the current available Show for this Body".into(),
        );
    }
    Ok(json!({"command":receipt,"input":input,"evidence":evidence,
        "observed_at_unix_ms":observed,"todo_actions_executed":0}))
}
