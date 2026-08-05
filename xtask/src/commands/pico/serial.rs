use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

use super::{PicoArgs, PicoResult};

const EXPECTED_RECEIPTS: usize = 16;
const DEVICE_ID_NEEDLE: &str = "conduit-pico-w-signal";

pub fn run_verify(args: &PicoArgs) -> PicoResult<()> {
    if args.dry_run {
        println!("==> pico verify (dry-run)");
        println!(
            "  serial port: {}",
            args.port
                .as_deref()
                .unwrap_or("<auto-discover conduit-pico-w-signal>")
        );
        return Ok(());
    }

    let port = resolve_port(args)?;
    println!("==> pico verify: reading receipts from {}", port.display());

    let _ = Command::new("stty")
        .args([
            "-F",
            port.to_str().ok_or("serial path is not UTF-8")?,
            "115200",
            "cs8",
            "-cstopb",
            "-parenb",
            "raw",
            "-echo",
        ])
        .status();

    let file = std::fs::OpenOptions::new().read(true).open(&port)?;
    verify_receipts(BufReader::new(file))
}

fn verify_receipts(reader: impl BufRead) -> PicoResult<()> {
    let mut receipts = Vec::new();
    let mut terminal_seen = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("malformed receipt JSON: {error}; line: {line}"))?;
        let schema = record["schema"].as_str().unwrap_or_default();

        if schema.starts_with("conduit-pico-w-signal/terminal") {
            if record["success"].as_bool() != Some(true) {
                return Err(format!("firmware reported terminal failure: {line}").into());
            }
            terminal_seen = true;
            break;
        }

        if schema.starts_with("conduit-pico-w-signal/receipt") {
            let sequence = record["sequence"]
                .as_u64()
                .ok_or("receipt missing sequence")?;
            let expected = receipts.len() as u64;
            if sequence != expected {
                return Err(format!(
                    "out-of-order receipt: expected sequence {expected}, got {sequence}"
                )
                .into());
            }
            receipts.push(record);
        }
    }

    if receipts.len() != EXPECTED_RECEIPTS {
        return Err(format!(
            "expected {EXPECTED_RECEIPTS} receipts, got {}",
            receipts.len()
        )
        .into());
    }
    if !terminal_seen {
        return Err("no successful terminal completion record received".into());
    }

    for (index, receipt) in receipts.iter().enumerate() {
        let expected_level = index % 2 == 1;
        let level = receipt["level"]
            .as_bool()
            .ok_or_else(|| format!("receipt {index} missing level"))?;
        if level != expected_level {
            return Err(format!(
                "receipt {index}: expected level={expected_level}, got level={level}"
            )
            .into());
        }
    }

    println!("==> pico verify: all {EXPECTED_RECEIPTS} receipts valid");
    Ok(())
}

fn resolve_port(args: &PicoArgs) -> PicoResult<PathBuf> {
    if let Some(port) = &args.port {
        let path = PathBuf::from(port);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("serial port does not exist: {}", path.display()).into());
    }

    let by_id = PathBuf::from("/dev/serial/by-id");
    if by_id.is_dir() {
        let matches = std::fs::read_dir(&by_id)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(DEVICE_ID_NEEDLE)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        return match matches.len() {
            1 => Ok(matches[0].clone()),
            0 => Err(
                "no Conduit Pico W serial device found; pass --port after connecting the board"
                    .into(),
            ),
            count => Err(format!(
                "{count} matching serial devices found under {}; pass --port",
                by_id.display()
            )
            .into()),
        };
    }

    Err("/dev/serial/by-id is unavailable; pass --port explicitly".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn receipt(sequence: usize) -> String {
        format!(
            "{{\"schema\":\"conduit-pico-w-signal/receipt@1\",\"sequence\":{sequence},\"level\":{}}}\n",
            sequence % 2 == 1
        )
    }

    #[test]
    fn accepts_exact_sixteen_receipts_and_terminal() {
        let mut input = String::new();
        for sequence in 0..EXPECTED_RECEIPTS {
            input.push_str(&receipt(sequence));
        }
        input.push_str("{\"schema\":\"conduit-pico-w-signal/terminal@1\",\"success\":true}\n");
        verify_receipts(Cursor::new(input)).expect("valid receipt stream");
    }

    #[test]
    fn rejects_reordered_receipt() {
        let input = format!(
            "{}{}",
            receipt(1),
            "{\"schema\":\"conduit-pico-w-signal/terminal@1\",\"success\":true}\n"
        );
        assert!(verify_receipts(Cursor::new(input)).is_err());
    }
}
