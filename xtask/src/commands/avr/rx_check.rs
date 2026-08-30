use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use clap::Args;
use serde::Serialize;

use super::{
    cdc_verify::{configure_serial, exchange, expected_attestation, expected_hello, read_line},
    run_build, verify_device, write_receipt, PhysicalGate, EXPECTED_BY_ID,
};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const SAMPLE_COUNT: u16 = 2048;
const REPLY_PREFIX: &str = "RX_BOUNDARY schema=conduit.pete/create-rx-boundary@1 outcome=sampled";
const REPLY_SUFFIX: &str = "rx_pin=D0/PD2 tx_pin=D1/PD3-input usart1=disabled create_tx_bytes=0";

#[derive(Args, Debug)]
pub(super) struct RxCheckArgs {
    #[arg(long)]
    port: PathBuf,
    #[arg(long)]
    artifact_sha256: String,
    #[arg(long)]
    create_stopped: bool,
    #[arg(long)]
    attended: bool,
    #[arg(long)]
    wheels_clear: bool,
    #[arg(long, default_value = "target/avr-promicro/rx-check-receipt.json")]
    receipt: PathBuf,
}

#[derive(Debug, Serialize)]
struct RxCheckReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    source_digest_sha256: String,
    build_id: String,
    artifact_sha256: String,
    port: String,
    samples: u16,
    high_samples: u16,
    low_samples: u16,
    transitions: u16,
    duration_us: u32,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    create_uart: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
struct Evidence {
    high: u16,
    low: u16,
    transitions: u16,
    duration_us: u32,
}

pub(super) fn run(args: RxCheckArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let gate = PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    };
    validate_request(&args, gate)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would rebuild and verify isolated artifact {}, verify {}, open CDC once, attest the exact image, and sample Create RX {} times with USART1 disabled and TX as input",
                args.artifact_sha256,
                args.port.display(),
                SAMPLE_COUNT
            );
        }
        return Ok(());
    }

    let built = run_build(
        Path::new("target/avr-promicro/build-receipt.json"),
        false,
        opts,
    )?;
    if built.artifact_sha256 != args.artifact_sha256 {
        return Err(format!(
            "AVR isolated artifact digest mismatch: expected {}, built {}",
            args.artifact_sha256, built.artifact_sha256
        )
        .into());
    }

    verify_device(&args.port)?;
    configure_serial(&args.port)?;
    let mut device = OpenOptions::new().read(true).write(true).open(&args.port)?;
    exchange(&mut device, "HELLO\n", expected_hello())?;
    exchange(
        &mut device,
        "ATTEST\n",
        &expected_attestation(&built.identity),
    )?;
    use std::io::Write as _;
    device.write_all(b"RXDIAG\n")?;
    device.flush()?;
    let evidence = parse_reply(&read_line(&mut device)?)?;

    let outcome = if evidence.high == SAMPLE_COUNT && evidence.low == 0 && evidence.transitions == 0
    {
        "stable-high"
    } else {
        "not-stable-high"
    };
    let root = workspace_root()?;
    let record = RxCheckReceipt {
        schema: "conduit.avr-promicro/create-rx-check@1",
        outcome,
        proof_class: "physical-gpio-receive-only",
        source_sha: built.identity.source_sha,
        source_digest_sha256: built.identity.source_digest_sha256,
        build_id: built.identity.build_id,
        artifact_sha256: built.artifact_sha256,
        port: args.port.display().to_string(),
        samples: SAMPLE_COUNT,
        high_samples: evidence.high,
        low_samples: evidence.low,
        transitions: evidence.transitions,
        duration_us: evidence.duration_us,
        create_stopped: gate.create_stopped,
        attended: gate.attended,
        wheels_clear: gate.wheels_clear,
        create_uart: "isolated-no-transmitter",
    };
    write_receipt(&root.join(args.receipt), &record, opts)?;
    if outcome != "stable-high" {
        return Err(format!(
            "Create RX boundary was not stable high: high={} low={} transitions={}",
            evidence.high, evidence.low, evidence.transitions
        )
        .into());
    }
    Ok(())
}

fn validate_request(
    args: &RxCheckArgs,
    gate: PhysicalGate,
) -> Result<(), Box<dyn std::error::Error>> {
    gate.validate("receive-only Create RX check")?;
    if args.port.file_name().and_then(|name| name.to_str()) != Some(EXPECTED_BY_ID)
        || args.port.parent() != Some(Path::new("/dev/serial/by-id"))
    {
        return Err(format!(
            "AVR receive-only Create RX check requires exact path /dev/serial/by-id/{EXPECTED_BY_ID}"
        )
        .into());
    }
    if args.artifact_sha256.len() != 64
        || !args
            .artifact_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("AVR receive-only Create RX check requires one exact SHA-256 digest".into());
    }
    Ok(())
}

fn parse_reply(reply: &str) -> Result<Evidence, Box<dyn std::error::Error>> {
    let mut fields = reply.split_ascii_whitespace();
    if fields.next() != Some("RX_BOUNDARY")
        || fields.next() != Some("schema=conduit.pete/create-rx-boundary@1")
        || fields.next() != Some("outcome=sampled")
    {
        return Err("unexpected Create RX boundary reply prefix".into());
    }
    let samples = parse_field::<u16>(fields.next(), "samples")?;
    let evidence = Evidence {
        high: parse_field(fields.next(), "high")?,
        low: parse_field(fields.next(), "low")?,
        transitions: parse_field(fields.next(), "transitions")?,
        duration_us: parse_field(fields.next(), "duration_us")?,
    };
    if samples != SAMPLE_COUNT || evidence.high.checked_add(evidence.low) != Some(SAMPLE_COUNT) {
        return Err("invalid Create RX boundary sample accounting".into());
    }
    let suffix = fields.collect::<Vec<_>>().join(" ");
    if suffix != REPLY_SUFFIX || !reply.starts_with(REPLY_PREFIX) {
        return Err("unexpected Create RX boundary isolation evidence".into());
    }
    Ok(evidence)
}

fn parse_field<T>(field: Option<&str>, name: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + 'static,
{
    let value = field
        .and_then(|field| field.strip_prefix(name))
        .and_then(|field| field.strip_prefix('='))
        .ok_or_else(|| format!("missing Create RX boundary field {name}"))?;
    Ok(value.parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_stable_high_reply_parses() {
        let reply = format!(
            "{REPLY_PREFIX} samples=2048 high=2048 low=0 transitions=0 duration_us=800 {REPLY_SUFFIX}"
        );
        assert_eq!(
            parse_reply(&reply).unwrap(),
            Evidence {
                high: 2048,
                low: 0,
                transitions: 0,
                duration_us: 800
            }
        );
    }

    #[test]
    fn malformed_or_unaccounted_reply_is_rejected() {
        let wrong = format!(
            "{REPLY_PREFIX} samples=2048 high=2047 low=0 transitions=0 duration_us=800 {REPLY_SUFFIX}"
        );
        assert!(parse_reply(&wrong).is_err());
        assert!(parse_reply("RX_BOUNDARY outcome=sampled").is_err());
    }
}
