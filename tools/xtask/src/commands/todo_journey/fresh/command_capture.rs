//! Finite command evidence for the fresh installed-owner producer.
use super::super::{retain, sha};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

// Match the installed owner's finite retained-state envelope. Its full Host
// inventory currently makes the initial Birth response exceed the terminal
// encounter's independent 512 KiB transcript bound.
pub(super) const MAX_COMMAND_OUTPUT: usize = 2 * 1024 * 1024;
const OVERFLOW_PREFIX_BYTES: usize = 8192;

fn retain_streams(root: &Path, label: &str, stdout: &[u8], stderr: &[u8]) -> Result<Value, String> {
    if stdout.len() > MAX_COMMAND_OUTPUT || stderr.len() > MAX_COMMAND_OUTPUT {
        let mut streams = Vec::new();
        for (name, bytes) in [("stdout", stdout), ("stderr", stderr)] {
            let prefix = retain(
                root,
                &format!("{label}.{name}.prefix"),
                &bytes[..bytes.len().min(OVERFLOW_PREFIX_BYTES)],
            )?;
            streams.push(
                json!({"stream":name,"bytes":bytes.len(),"sha256":sha(bytes),"prefix":prefix}),
            );
        }
        retain(
            root,
            &format!("{label}.output-bound.json"),
            &serde_json::to_vec_pretty(&json!({
                "schema":"conduit.proof/command-output-overflow@1",
                "maximum_stream_bytes":MAX_COMMAND_OUTPUT,"streams":streams,
            }))
            .map_err(|error| error.to_string())?,
        )?;
        return Err(format!("{label} command output exceeds capture bound; lengths, hashes and bounded prefixes retained"));
    }
    let mut streams = Vec::new();
    for (name, bytes) in [("stdout", stdout), ("stderr", stderr)] {
        let name = format!("{label}.{name}");
        fs::write(root.join(&name), bytes).map_err(|error| error.to_string())?;
        streams.push(json!({"path":name,"bytes":bytes.len(),"sha256":sha(bytes)}));
    }
    Ok(json!({"stdout":streams[0],"stderr":streams[1]}))
}

pub(super) fn capture(
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
    let mut evidence = retain_streams(root, label, &output.stdout, &output.stderr)?;
    if !output.status.success() {
        return Err(format!("{label} refused; raw stdout and stderr retained"));
    }
    evidence["exit_code"] = json!(output.status.code());
    Ok(evidence)
}

pub(super) fn json_output(
    root: &Path,
    label: &str,
    command: &mut Command,
) -> Result<Value, String> {
    capture(root, label, command, None)?;
    let bytes =
        fs::read(root.join(format!("{label}.stdout"))).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label} returned invalid JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn directory() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "todo-command-output-{}-{}-{}",
            std::process::id(),
            super::super::now().unwrap(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn fresh_owner_inventory_above_terminal_bound_is_retained_byte_exactly() {
        let root = directory();
        let bytes = vec![b'x'; 922_259];
        let evidence = retain_streams(&root, "birth", &bytes, b"diagnostic").unwrap();
        assert_eq!(fs::read(root.join("birth.stdout")).unwrap(), bytes);
        assert_eq!(fs::read(root.join("birth.stderr")).unwrap(), b"diagnostic");
        assert_eq!(evidence["stdout"]["bytes"], 922_259);
        let exact = vec![b'y'; MAX_COMMAND_OUTPUT];
        retain_streams(&root, "boundary", &exact, b"").unwrap();
        assert_eq!(fs::read(root.join("boundary.stdout")).unwrap(), exact);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn overbound_output_refuses_with_lengths_hashes_and_only_bounded_prefixes() {
        let root = directory();
        let bytes = vec![b'z'; MAX_COMMAND_OUTPUT + 1];
        assert!(retain_streams(&root, "birth", &bytes, b"failure detail").is_err());
        assert!(!root.join("birth.stdout").exists());
        assert_eq!(
            fs::read(root.join("birth.stdout.prefix")).unwrap().len(),
            OVERFLOW_PREFIX_BYTES
        );
        assert_eq!(
            fs::read(root.join("birth.stderr.prefix")).unwrap(),
            b"failure detail"
        );
        let evidence: Value =
            serde_json::from_slice(&fs::read(root.join("birth.output-bound.json")).unwrap())
                .unwrap();
        assert_eq!(evidence["streams"][0]["bytes"], MAX_COMMAND_OUTPUT + 1);
        assert_eq!(evidence["streams"][0]["sha256"], sha(&bytes));
        fs::remove_dir_all(root).unwrap();
    }
}
