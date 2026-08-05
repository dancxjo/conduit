use anyhow::{bail, Result};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

use super::PicoArgs;

const EXPECTED_RECEIPTS: usize = 16;
const DEVICE_ID_NEEDLE: &str = "conduit-pico-w-signal";

pub fn run_verify(args: &PicoArgs) -> Result<()> {
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

    // Configure the serial port via stty for 115200 baud raw mode
    let stty_status = Command::new("stty")
        .args([
            "-F",
            port.to_str().unwrap(),
            "115200",
            "cs8",
            "-cstopb",
            "-parenb",
            "raw",
            "-echo",
        ])
        .status();
    if let Ok(status) = stty_status {
        if !status.success() {
            println!("  warning: stty configuration returned non-zero (may still work)");
        }
    }

    // Open the serial port as a regular file
    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(&port)
        .map_err(|e| anyhow::anyhow!("cannot open serial port {}: {}", port.display(), e))?;

    // Set a read timeout using a background thread
    let reader = BufReader::new(file);
    verify_receipts(reader)
}

fn verify_receipts(reader: impl BufRead) -> Result<()> {
    let mut receipts: Vec<serde_json::Value> = Vec::new();
    let mut terminal_seen = false;

    for line in reader.lines() {
        let line = line.map_err(|e| anyhow::anyhow!("serial read error: {}", e))?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| anyhow::anyhow!("malformed receipt JSON: {}\nline: {}", e, line))?;

        let schema = record["schema"].as_str().unwrap_or("").to_string();

        if schema.starts_with("conduit-pico-w-signal/terminal") {
            terminal_seen = true;
            println!("  terminal: {}", line);
            break;
        }

        if schema.starts_with("conduit-pico-w-signal/receipt") {
            let seq = record["sequence"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("receipt missing sequence"))?;
            let expected_seq = receipts.len() as u64;
            if seq != expected_seq {
                bail!(
                    "out-of-order receipt: expected sequence {}, got {}",
                    expected_seq,
                    seq
                );
            }
            receipts.push(record);
        }
    }

    if receipts.len() != EXPECTED_RECEIPTS {
        bail!(
            "expected {} receipts, got {}",
            EXPECTED_RECEIPTS,
            receipts.len()
        );
    }
    if !terminal_seen {
        bail!("no terminal completion record received");
    }

    // Verify signal levels: initial=false, alternating thereafter
    let mut expected_level = false;
    for (i, receipt) in receipts.iter().enumerate() {
        let level = receipt["level"]
            .as_bool()
            .ok_or_else(|| anyhow::anyhow!("receipt {} missing level", i))?;
        if level != expected_level {
            bail!(
                "receipt {}: expected level={}, got level={}",
                i,
                expected_level,
                level
            );
        }
        expected_level = !expected_level;
    }

    println!("==> pico verify: all {} receipts valid", EXPECTED_RECEIPTS);
    Ok(())
}

fn resolve_port(args: &PicoArgs) -> Result<PathBuf> {
    if let Some(p) = &args.port {
        return Ok(PathBuf::from(p));
    }

    // Discover via /dev/serial/by-id
    let by_id = PathBuf::from("/dev/serial/by-id");
    if by_id.is_dir() {
        let matches: Vec<_> = std::fs::read_dir(&by_id)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains(DEVICE_ID_NEEDLE)
            })
            .collect();
        match matches.len() {
            0 => {}
            1 => return Ok(matches[0].path()),
            n => bail!(
                "{} matching serial devices found under {}. Use --port to specify one.",
                n,
                by_id.display()
            ),
        }
    }

    bail!(
        "No Conduit Pico W serial device found. Ensure the Pico W is running and connected, \
         then retry, or pass --port <path>."
    )
}
