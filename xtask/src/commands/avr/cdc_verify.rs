use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use clap::Args;
use serde::Serialize;

use super::{require_success, verify_device, write_receipt, PhysicalGate, EXPECTED_BY_ID};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const MAX_REPLY_BYTES: usize = 512;
const REPLY_DEADLINE: Duration = Duration::from_secs(3);

#[derive(Args, Debug)]
pub(super) struct VerifyArgs {
    #[arg(long)]
    port: PathBuf,
    #[arg(long, value_parser = parse_u32_hex)]
    host_id: u32,
    #[arg(long, value_parser = parse_u32_hex)]
    offer_generation: u32,
    #[arg(long, value_parser = parse_u32_hex)]
    plan_fragment_id: u32,
    #[arg(long, value_parser = parse_u16_hex)]
    operation_id: u16,
    #[arg(long, value_parser = parse_u32_hex)]
    active_play_id: u32,
    #[arg(long, value_parser = parse_u32_hex)]
    authority_grant_id: u32,
    #[arg(long)]
    create_stopped: bool,
    #[arg(long)]
    attended: bool,
    #[arg(long)]
    wheels_clear: bool,
    #[arg(long, default_value = "target/avr-promicro/cdc-verify-receipt.json")]
    receipt: PathBuf,
}

#[derive(Debug, Serialize)]
struct VerifyReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    port: String,
    host_id: String,
    boot_id: String,
    offer_generation: String,
    plan_fragment_id: String,
    operation_id: String,
    active_play_id: String,
    authority_grant_id: String,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    execution: &'static str,
    create_uart: &'static str,
}

#[derive(Clone, Copy)]
struct Identities {
    host: u32,
    boot: u32,
    offer: u32,
    fragment: u32,
    operation: u16,
    play: u32,
    grant: u32,
}

pub(super) fn run(args: VerifyArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let gate = PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    };
    validate_request(&args.port, gate, &args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would verify {}, open CDC once, bind a fresh Boot, admit one disabled observation activation, and require isolated status",
                args.port.display()
            );
        }
        return Ok(());
    }

    verify_device(&args.port)?;
    configure_serial(&args.port)?;
    let boot = fresh_boot_id()?;
    let identities = Identities {
        host: args.host_id,
        boot,
        offer: args.offer_generation,
        fragment: args.plan_fragment_id,
        operation: args.operation_id,
        play: args.active_play_id,
        grant: args.authority_grant_id,
    };
    let mut device = OpenOptions::new().read(true).write(true).open(&args.port)?;
    exchange(&mut device, "HELLO\n", expected_hello())?;
    exchange(
        &mut device,
        &boot_frame(identities),
        &expected_boot(identities),
    )?;
    exchange(
        &mut device,
        &activation_frame(identities),
        &expected_activation(identities),
    )?;
    exchange(&mut device, "STATUS\n", expected_status())?;

    let root = workspace_root()?;
    let record = VerifyReceipt {
        schema: "conduit.avr-promicro/cdc-verify@1",
        outcome: "verified",
        proof_class: "physical-usb-cdc-fail-closed",
        source_sha: super::git_head(&root)?,
        port: args.port.display().to_string(),
        host_id: hex32(identities.host),
        boot_id: hex32(identities.boot),
        offer_generation: hex32(identities.offer),
        plan_fragment_id: hex32(identities.fragment),
        operation_id: hex16(identities.operation),
        active_play_id: hex32(identities.play),
        authority_grant_id: hex32(identities.grant),
        create_stopped: gate.create_stopped,
        attended: gate.attended,
        wheels_clear: gate.wheels_clear,
        execution: "disabled",
        create_uart: "isolated-no-transmitter",
    };
    write_receipt(&root.join(args.receipt), &record, opts)
}

fn validate_request(
    port: &Path,
    gate: PhysicalGate,
    args: &VerifyArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    gate.validate("CDC verification")?;
    if port.file_name().and_then(|name| name.to_str()) != Some(EXPECTED_BY_ID)
        || port.parent() != Some(Path::new("/dev/serial/by-id"))
    {
        return Err(format!(
            "AVR CDC verification requires exact path /dev/serial/by-id/{EXPECTED_BY_ID}"
        )
        .into());
    }
    if [
        args.host_id,
        args.offer_generation,
        args.plan_fragment_id,
        u32::from(args.operation_id),
        args.active_play_id,
        args.authority_grant_id,
    ]
    .contains(&0)
    {
        return Err("AVR CDC verification identities must all be nonzero".into());
    }
    Ok(())
}

fn configure_serial(port: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("stty")
        .args(["-F"])
        .arg(port)
        .args(["115200", "raw", "-echo", "min", "0", "time", "20"])
        .output()?;
    require_success(&output, "Pro Micro CDC configuration")
}

fn fresh_boot_id() -> Result<u32, Box<dyn std::error::Error>> {
    let mut bytes = [0_u8; 4];
    OpenOptions::new()
        .read(true)
        .open("/dev/urandom")?
        .read_exact(&mut bytes)?;
    let value = u32::from_be_bytes(bytes);
    if value == 0 {
        return Err("fresh AVR Boot identity was zero".into());
    }
    Ok(value)
}

fn exchange(
    device: &mut impl ReadWrite,
    request: &str,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    device.write_all(request.as_bytes())?;
    device.flush()?;
    let actual = read_line(device)?;
    if actual != expected {
        return Err(
            format!("AVR CDC reply mismatch: expected {expected:?}, received {actual:?}").into(),
        );
    }
    Ok(())
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

fn read_line(device: &mut impl Read) -> Result<String, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut bytes = Vec::with_capacity(128);
    let mut byte = [0_u8; 1];
    while start.elapsed() < REPLY_DEADLINE {
        match device.read(&mut byte)? {
            0 => continue,
            1 if byte[0] == b'\n' => return Ok(String::from_utf8(bytes)?),
            1 => {
                if bytes.len() == MAX_REPLY_BYTES {
                    return Err("AVR CDC reply exceeded 512-byte capacity".into());
                }
                bytes.push(byte[0]);
            }
            _ => unreachable!(),
        }
    }
    Err("AVR CDC reply deadline expired".into())
}

fn boot_frame(ids: Identities) -> String {
    format!(
        "B {}:{}:{}\n",
        hex32(ids.host),
        hex32(ids.boot),
        hex32(ids.offer)
    )
}

fn activation_frame(ids: Identities) -> String {
    format!(
        "A {}:{}:{}:{}:{}:{}:{}\n",
        hex32(ids.host),
        hex32(ids.boot),
        hex32(ids.offer),
        hex32(ids.fragment),
        hex16(ids.operation),
        hex32(ids.play),
        hex32(ids.grant)
    )
}

fn expected_hello() -> &'static str {
    "TARGET schema=conduit.target/availability@1 target_id=avr/promicro/pete-brainstem target=atmega32u4-5v-16mhz line=usb-cdc@1"
}

fn expected_boot(ids: Identities) -> String {
    format!(
        "BOOT_BIND schema=conduit.host/boot-binding@1 outcome=bound host={} boot={} offer_generation={} create_uart=isolated",
        hex32(ids.host), hex32(ids.boot), hex32(ids.offer)
    )
}

fn expected_activation(ids: Identities) -> String {
    format!(
        "ACTIVATION schema=conduit.host/observation-activation@1 outcome=admitted host={} boot={} offer_generation={} plan_fragment={} operation={} active_play={} authority_grant={} execution=disabled create_uart=isolated",
        hex32(ids.host),
        hex32(ids.boot),
        hex32(ids.offer),
        hex32(ids.fragment),
        hex16(ids.operation),
        hex32(ids.play),
        hex32(ids.grant)
    )
}

fn expected_status() -> &'static str {
    "STATUS schema=conduit.pete/promicro-brainstem@1 create_uart=isolated create_tx_bytes=0 boot_binding=bound activation=admitted motion_authority=absent command_capacity=64 assigned_obligation_capacity=1 group_zero_bytes=26 create_codec=compiled-disabled"
}

fn hex32(value: u32) -> String {
    format!("{value:08X}")
}

fn hex16(value: u16) -> String {
    format!("{value:04X}")
}

fn parse_u32_hex(value: &str) -> Result<u32, String> {
    parse_hex(value, 8).map(|parsed| parsed as u32)
}

fn parse_u16_hex(value: &str) -> Result<u16, String> {
    parse_hex(value, 4).map(|parsed| parsed as u16)
}

fn parse_hex(value: &str, maximum_digits: usize) -> Result<u64, String> {
    if value.is_empty()
        || value.len() > maximum_digits
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!("expected 1 to {maximum_digits} hexadecimal digits"));
    }
    u64::from_str_radix(value, 16).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Identities {
        Identities {
            host: 11,
            boot: 22,
            offer: 1,
            fragment: 33,
            operation: 44,
            play: 55,
            grant: 66,
        }
    }

    fn args() -> VerifyArgs {
        VerifyArgs {
            port: PathBuf::from("/dev/serial/by-id/usb-SparkFun_SparkFun_Pro_Micro-if00"),
            host_id: 11,
            offer_generation: 1,
            plan_fragment_id: 33,
            operation_id: 44,
            active_play_id: 55,
            authority_grant_id: 66,
            create_stopped: true,
            attended: true,
            wheels_clear: true,
            receipt: PathBuf::from("unused.json"),
        }
    }

    #[test]
    fn frames_are_exact_bounded_and_canonical() {
        assert_eq!(boot_frame(ids()), "B 0000000B:00000016:00000001\n");
        assert_eq!(
            activation_frame(ids()),
            "A 0000000B:00000016:00000001:00000021:002C:00000037:00000042\n"
        );
        assert!(boot_frame(ids()).len() <= 65);
        assert!(activation_frame(ids()).len() <= 65);
    }

    #[test]
    fn expected_transcript_requires_isolation_and_exact_identities() {
        assert!(expected_boot(ids()).contains("outcome=bound"));
        assert!(expected_activation(ids())
            .contains("authority_grant=00000042 execution=disabled create_uart=isolated"));
        assert!(expected_status().contains("create_tx_bytes=0"));
        assert!(expected_status().contains("motion_authority=absent"));
    }

    #[test]
    fn identity_parser_is_finite_and_typed() {
        assert_eq!(parse_u32_hex("FFFFFFFF").unwrap(), u32::MAX);
        assert_eq!(parse_u16_hex("FFFF").unwrap(), u16::MAX);
        assert!(parse_u32_hex("100000000").is_err());
        assert!(parse_u16_hex("10000").is_err());
        assert!(parse_u32_hex("").is_err());
    }

    #[test]
    fn physical_gates_and_exact_by_id_are_checked_before_open() {
        let mut request = args();
        let gate = PhysicalGate {
            create_stopped: true,
            attended: true,
            wheels_clear: false,
        };
        assert!(validate_request(&request.port, gate, &request)
            .unwrap_err()
            .to_string()
            .contains("--wheels-clear"));
        request.port = PathBuf::from("/dev/ttyACM0");
        let complete = PhysicalGate {
            create_stopped: true,
            attended: true,
            wheels_clear: true,
        };
        assert!(validate_request(&request.port, complete, &request).is_err());
        request.port = PathBuf::from("/dev/serial/by-id/usb-SparkFun_SparkFun_Pro_Micro-if00");
        request.host_id = 0;
        assert!(validate_request(&request.port, complete, &request).is_err());
    }
}
