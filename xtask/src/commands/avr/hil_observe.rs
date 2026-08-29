use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use clap::Args;
use serde::Serialize;

use super::{
    cdc_verify::{
        activation_frame, boot_frame, configure_serial, exchange, expected_activation,
        expected_boot, expected_hello, fresh_boot_id, hex16, hex32, read_line, Identities,
    },
    run_build, verify_device, write_receipt, PhysicalGate, EXPECTED_BY_ID,
};
use crate::{cli::GlobalOpts, workspace::workspace_root};

const MAX_DEADLINE_MS: u16 = 2000;

#[derive(Args, Debug)]
pub(super) struct ObserveArgs {
    #[arg(long)]
    port: PathBuf,
    #[arg(long)]
    artifact_sha256: String,
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
    #[arg(long, default_value_t = 500)]
    deadline_ms: u16,
    #[arg(long)]
    create_stopped: bool,
    #[arg(long)]
    attended: bool,
    #[arg(long)]
    wheels_clear: bool,
    #[arg(long, default_value = "target/avr-promicro/hil-observe-receipt.json")]
    receipt: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct GroupZeroSample {
    bump_and_wheel_drop: u8,
    wall: bool,
    cliff_bits: u8,
    virtual_wall: bool,
    wheel_overcurrents: u8,
    dirt_detect: u16,
    infrared: u8,
    buttons: u8,
    distance_delta_mm: i16,
    angle_delta_degrees: i16,
    charging_state: u8,
    millivolts: u16,
    milliamps: i16,
    temperature_celsius: i8,
    charge_mah: u16,
    capacity_mah: u16,
}

#[derive(Debug, Serialize)]
struct ObserveReceipt {
    schema: &'static str,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    artifact_sha256: String,
    port: String,
    host_id: String,
    boot_id: String,
    offer_generation: String,
    plan_fragment_id: String,
    operation_id: String,
    active_play_id: String,
    authority_grant_id: String,
    deadline_ms: u16,
    setup_bytes: u8,
    request_bytes: u8,
    response_bytes: u8,
    create_tx_bytes: u8,
    create_stopped: bool,
    attended: bool,
    wheels_clear: bool,
    sample: GroupZeroSample,
    create_uart: &'static str,
    motion_authority: &'static str,
}

pub(super) fn run(args: ObserveArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let gate = PhysicalGate {
        create_stopped: args.create_stopped,
        attended: args.attended,
        wheels_clear: args.wheels_clear,
    };
    validate_request(&args, gate)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would rebuild and verify HIL artifact {}, verify {}, bind one fresh Boot, execute bytes 128,132,142,0 once, require one valid 26-byte sample, and restore UART isolation",
                args.artifact_sha256,
                args.port.display()
            );
        }
        return Ok(());
    }

    let (_, built_digest) = run_build(
        Path::new("target/avr-promicro/build-hil-receipt.json"),
        true,
        opts,
    )?;
    if built_digest != args.artifact_sha256 {
        return Err(format!(
            "AVR HIL artifact digest mismatch: expected {}, built {built_digest}",
            args.artifact_sha256
        )
        .into());
    }

    verify_device(&args.port)?;
    configure_serial(&args.port)?;
    let identities = Identities {
        host: args.host_id,
        boot: fresh_boot_id()?,
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
    exchange(&mut device, "STATUS\n", &expected_hil_status(0))?;
    exchange(
        &mut device,
        &execution_frame(identities, args.deadline_ms),
        &expected_execution(identities, args.deadline_ms),
    )?;
    let terminal = read_line(&mut device)?;
    let sample = parse_terminal(&terminal, identities)?;
    exchange(&mut device, "STATUS\n", &expected_hil_status(4))?;

    let root = workspace_root()?;
    let record = ObserveReceipt {
        schema: "conduit.avr-promicro/create-group-zero@1",
        outcome: "completed",
        proof_class: "physical-create-group-zero-observation",
        source_sha: super::git_head(&root)?,
        artifact_sha256: built_digest,
        port: args.port.display().to_string(),
        host_id: hex32(identities.host),
        boot_id: hex32(identities.boot),
        offer_generation: hex32(identities.offer),
        plan_fragment_id: hex32(identities.fragment),
        operation_id: hex16(identities.operation),
        active_play_id: hex32(identities.play),
        authority_grant_id: hex32(identities.grant),
        deadline_ms: args.deadline_ms,
        setup_bytes: 2,
        request_bytes: 2,
        response_bytes: 26,
        create_tx_bytes: 4,
        create_stopped: gate.create_stopped,
        attended: gate.attended,
        wheels_clear: gate.wheels_clear,
        sample,
        create_uart: "isolated-after-terminal",
        motion_authority: "absent",
    };
    write_receipt(&root.join(args.receipt), &record, opts)
}

fn validate_request(
    args: &ObserveArgs,
    gate: PhysicalGate,
) -> Result<(), Box<dyn std::error::Error>> {
    gate.validate("Create HIL observation")?;
    if args.port.file_name().and_then(|name| name.to_str()) != Some(EXPECTED_BY_ID)
        || args.port.parent() != Some(Path::new("/dev/serial/by-id"))
    {
        return Err(format!(
            "AVR Create HIL observation requires exact path /dev/serial/by-id/{EXPECTED_BY_ID}"
        )
        .into());
    }
    if args.artifact_sha256.len() != 64
        || !args
            .artifact_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("AVR Create HIL observation requires one exact SHA-256 digest".into());
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
        return Err("AVR Create HIL observation identities must all be nonzero".into());
    }
    if args.deadline_ms == 0 || args.deadline_ms > MAX_DEADLINE_MS {
        return Err(format!(
            "AVR Create HIL observation deadline must be 1..={MAX_DEADLINE_MS} ms"
        )
        .into());
    }
    Ok(())
}

fn execution_frame(ids: Identities, deadline_ms: u16) -> String {
    format!(
        "O {}:{}:{}\n",
        hex32(ids.fragment),
        hex16(ids.operation),
        hex16(deadline_ms)
    )
}

fn expected_execution(ids: Identities, deadline_ms: u16) -> String {
    format!(
        "EXECUTION schema=conduit.pete/create-group-zero@1 outcome=started plan_fragment={} operation={} deadline_ms={} setup_bytes=2 request_bytes=2 response_capacity=26",
        hex32(ids.fragment),
        hex16(ids.operation),
        hex16(deadline_ms)
    )
}

fn expected_hil_status(create_tx_bytes: u8) -> String {
    format!(
        "STATUS schema=conduit.pete/promicro-brainstem@1 create_uart=isolated create_tx_bytes={create_tx_bytes} boot_binding=bound activation=admitted motion_authority=absent command_capacity=64 assigned_obligation_capacity=1 group_zero_bytes=26 create_codec=compiled-hil-isolated"
    )
}

fn parse_terminal(
    line: &str,
    ids: Identities,
) -> Result<GroupZeroSample, Box<dyn std::error::Error>> {
    let tokens: Vec<&str> = line.split_ascii_whitespace().collect();
    if tokens.len() != 24
        || tokens[0] != "TERMINAL"
        || tokens[1] != "schema=conduit.pete/create-group-zero@1"
        || tokens[2] != "outcome=completed"
        || field(&tokens, 3, "plan_fragment")? != hex32(ids.fragment)
        || field(&tokens, 4, "operation")? != hex16(ids.operation)
        || tokens[5] != "response_bytes=1A"
        || tokens[6] != "payload=valid"
        || tokens[23] != "create_uart=isolated"
    {
        return Err("AVR HIL terminal identity, disposition, or shape mismatch".into());
    }
    let sample = GroupZeroSample {
        bump_and_wheel_drop: hex_u8(field(&tokens, 7, "bump_drop")?)?,
        wall: boolean(field(&tokens, 8, "wall")?)?,
        cliff_bits: hex_u8(field(&tokens, 9, "cliffs")?)?,
        virtual_wall: boolean(field(&tokens, 10, "virtual_wall")?)?,
        wheel_overcurrents: hex_u8(field(&tokens, 11, "wheel_overcurrents")?)?,
        dirt_detect: hex_u16(field(&tokens, 12, "dirt")?)?,
        infrared: hex_u8(field(&tokens, 13, "infrared")?)?,
        buttons: hex_u8(field(&tokens, 14, "buttons")?)?,
        distance_delta_mm: hex_u16(field(&tokens, 15, "distance_mm")?)? as i16,
        angle_delta_degrees: hex_u16(field(&tokens, 16, "angle_degrees")?)? as i16,
        charging_state: hex_u8(field(&tokens, 17, "charging_state")?)?,
        millivolts: hex_u16(field(&tokens, 18, "millivolts")?)?,
        milliamps: hex_u16(field(&tokens, 19, "milliamps")?)? as i16,
        temperature_celsius: hex_u8(field(&tokens, 20, "temperature_c")?)? as i8,
        charge_mah: hex_u16(field(&tokens, 21, "charge_mah")?)?,
        capacity_mah: hex_u16(field(&tokens, 22, "capacity_mah")?)?,
    };
    if sample.bump_and_wheel_drop & !0x1f != 0
        || sample.cliff_bits & !0x0f != 0
        || sample.buttons & !0x05 != 0
        || sample.charging_state > 5
    {
        return Err("AVR HIL terminal sample violates Create group-zero ranges".into());
    }
    Ok(sample)
}

fn field<'a>(tokens: &'a [&str], index: usize, key: &str) -> Result<&'a str, String> {
    let (actual_key, value) = tokens[index]
        .split_once('=')
        .ok_or_else(|| format!("AVR HIL terminal field {index} omitted '='"))?;
    if actual_key != key {
        return Err(format!(
            "AVR HIL terminal field {index} expected {key}, got {actual_key}"
        ));
    }
    Ok(value)
}

fn boolean(value: &str) -> Result<bool, String> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("expected canonical boolean, got {value}")),
    }
}

fn hex_u8(value: &str) -> Result<u8, String> {
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("expected two hexadecimal digits, got {value}"));
    }
    u8::from_str_radix(value, 16).map_err(|error| error.to_string())
}

fn hex_u16(value: &str) -> Result<u16, String> {
    if value.len() != 4 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("expected four hexadecimal digits, got {value}"));
    }
    u16::from_str_radix(value, 16).map_err(|error| error.to_string())
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

    fn args() -> ObserveArgs {
        ObserveArgs {
            port: PathBuf::from("/dev/serial/by-id/usb-SparkFun_SparkFun_Pro_Micro-if00"),
            artifact_sha256: "a".repeat(64),
            host_id: 11,
            offer_generation: 1,
            plan_fragment_id: 33,
            operation_id: 44,
            active_play_id: 55,
            authority_grant_id: 66,
            deadline_ms: 500,
            create_stopped: true,
            attended: true,
            wheels_clear: true,
            receipt: PathBuf::from("unused.json"),
        }
    }

    fn terminal() -> &'static str {
        "TERMINAL schema=conduit.pete/create-group-zero@1 outcome=completed plan_fragment=00000021 operation=002C response_bytes=1A payload=valid bump_drop=1B wall=1 cliffs=05 virtual_wall=1 wheel_overcurrents=03 dirt=1234 infrared=89 buttons=05 distance_mm=FF88 angle_degrees=001E charging_state=03 millivolts=3778 milliamps=FF10 temperature_c=1F charge_mah=04B0 capacity_mah=0960 create_uart=isolated"
    }

    #[test]
    fn exact_execution_and_status_frames_are_bounded() {
        assert_eq!(execution_frame(ids(), 500), "O 00000021:002C:01F4\n");
        assert_eq!(
            expected_execution(ids(), 500),
            "EXECUTION schema=conduit.pete/create-group-zero@1 outcome=started plan_fragment=00000021 operation=002C deadline_ms=01F4 setup_bytes=2 request_bytes=2 response_capacity=26"
        );
        assert!(execution_frame(ids(), 500).len() <= 65);
        assert!(expected_hil_status(4).contains("create_tx_bytes=4"));
    }

    #[test]
    fn terminal_parser_preserves_every_group_zero_field() {
        let sample = parse_terminal(terminal(), ids()).unwrap();
        assert_eq!(sample.bump_and_wheel_drop, 0x1b);
        assert!(sample.wall);
        assert_eq!(sample.cliff_bits, 0x05);
        assert_eq!(sample.distance_delta_mm, -120);
        assert_eq!(sample.angle_delta_degrees, 30);
        assert_eq!(sample.millivolts, 14200);
        assert_eq!(sample.milliamps, -240);
        assert_eq!(sample.charge_mah, 1200);
        assert_eq!(sample.capacity_mah, 2400);
    }

    #[test]
    fn terminal_parser_rejects_stale_malformed_and_noncanonical_samples() {
        assert!(parse_terminal(
            &terminal().replace("operation=002C", "operation=002D"),
            ids()
        )
        .is_err());
        assert!(parse_terminal(&terminal().replace("wall=1", "wall=true"), ids()).is_err());
        assert!(parse_terminal(
            &terminal().replace("charging_state=03", "charging_state=06"),
            ids()
        )
        .is_err());
        assert!(parse_terminal(&format!("{} extra=1", terminal()), ids()).is_err());
    }

    #[test]
    fn safety_gates_digest_identity_and_deadline_precede_device_access() {
        let mut request = args();
        let missing_gate = PhysicalGate {
            create_stopped: true,
            attended: true,
            wheels_clear: false,
        };
        assert!(validate_request(&request, missing_gate)
            .unwrap_err()
            .to_string()
            .contains("--wheels-clear"));
        let complete = PhysicalGate {
            create_stopped: true,
            attended: true,
            wheels_clear: true,
        };
        request.artifact_sha256 = "bad".into();
        assert!(validate_request(&request, complete).is_err());
        request.artifact_sha256 = "a".repeat(64);
        request.deadline_ms = 0;
        assert!(validate_request(&request, complete).is_err());
        request.deadline_ms = MAX_DEADLINE_MS + 1;
        assert!(validate_request(&request, complete).is_err());
        request.deadline_ms = 500;
        request.operation_id = 0;
        assert!(validate_request(&request, complete).is_err());
    }
}
