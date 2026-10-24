//! Tool-only graphics inspection: no firmware, guest instructions or product frame.
use super::super::{qmp, ConduitosError};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct Evidence {
    pub display_backends: Vec<String>,
    pub screendump_command_available: bool,
    pub guest_instructions_executed: bool,
    pub product_frame_proved: bool,
}
impl Evidence {
    pub fn valid(&self) -> bool {
        self.display_backends.iter().any(|v| v == "sdl")
            && self.screendump_command_available
            && !self.guest_instructions_executed
            && !self.product_frame_proved
    }
}
fn decode(backends: &str, commands: &serde_json::Value) -> Result<Evidence, ConduitosError> {
    let mut lines = backends.lines();
    if lines.next() != Some("Available display backend types:") {
        return Err(super::refusal("invalid display backend inventory header"));
    }
    let backends = lines
        .map(str::trim)
        .take_while(|v| !v.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if backends.len() > 16
        || backends
            .iter()
            .any(|v| v.len() > 64 || v.contains(char::is_whitespace))
    {
        return Err(super::refusal("invalid display backend inventory"));
    }
    let commands = commands
        .as_array()
        .ok_or_else(|| super::refusal("invalid QMP command inventory"))?;
    let evidence = Evidence {
        display_backends: backends,
        screendump_command_available: commands.iter().any(|v| v["name"] == "screendump"),
        guest_instructions_executed: false,
        product_frame_proved: false,
    };
    if !evidence.valid() {
        return Err(super::refusal(
            "graphical diagnostic emulator requires SDL and QMP screendump",
        ));
    }
    Ok(evidence)
}
pub(super) fn probe(binary: &Path, destination: &Path) -> Result<Evidence, ConduitosError> {
    let backends = Command::new(binary)
        .args(["-display", "help"])
        .output()
        .map_err(super::refusal)?;
    if !backends.status.success() || backends.stdout.len() > 4096 {
        return Err(super::refusal("display backend inventory unavailable"));
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(super::refusal)?
        .as_nanos();
    let name = format!("clg-{}-{nonce}", std::process::id());
    let socket_dir = std::env::temp_dir().join(&name);
    let output = destination.join("graphics-probes").join(&name);
    fs::create_dir_all(&output).map_err(super::refusal)?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&socket_dir)
        .map_err(super::refusal)?;
    let socket = socket_dir.join("qmp");
    let mut child = Command::new(binary)
        .args([
            "-machine",
            "none",
            "-nodefaults",
            "-display",
            "none",
            "-monitor",
            "none",
            "-S",
            "-qmp",
        ])
        .arg(format!("unix:{},server=on,wait=off", socket.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let result = (|| {
        let child = child.as_mut().map_err(|e| super::refusal(e.to_string()))?;
        let (mut stream, mut reader) =
            qmp::connect_traced(&socket, child, Some(&output.join("qmp.jsonl")))?;
        let commands = qmp::request_value(
            &mut stream,
            &mut reader,
            br#"{"execute":"query-commands"}"#,
            "graphics-tool-commands",
        )?;
        let evidence = decode(&String::from_utf8_lossy(&backends.stdout), &commands)?;
        fs::write(
            output.join("evidence.json"),
            serde_json::to_vec_pretty(&evidence).map_err(super::refusal)?,
        )
        .map_err(super::refusal)?;
        Ok(evidence)
    })();
    if let Ok(child) = child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = fs::remove_file(socket);
    let _ = fs::remove_dir(socket_dir);
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_window_and_screenshot_command_are_independent_requirements() {
        let commands = serde_json::json!([{"name":"screendump"}]);
        let evidence = decode("Available display backend types:\nnone\nsdl\n", &commands).unwrap();
        assert!(!evidence.product_frame_proved && !evidence.guest_instructions_executed);
        assert!(decode("Available display backend types:\nnone\n", &commands).is_err());
        assert!(decode(
            "Available display backend types:\nnone\nsdl\n",
            &serde_json::json!([])
        )
        .is_err());
        assert!(decode("Available display backend types:\nnone\nsdl\ncurses\n\nSome display backends support suboptions, which can be set with\n   -display backend,option=value\n", &commands).is_ok());
        assert!(decode("wrong header\nnone\nsdl\n", &commands).is_err());
        let mut false_claim = evidence;
        false_claim.product_frame_proved = true;
        assert!(!false_claim.valid());
    }
}
