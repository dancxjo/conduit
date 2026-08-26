//! Exact two-Pico physical proof for the finite Hello appliance.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::appliance_identity::{
    read_appliance_hil_client_identity_manifest, read_appliance_identity_manifest,
};
use super::doctor::repo_root;
use super::firmware::uf2_path;
use super::prove_appliance::{
    physical_timestamp, read_complete_bounded_line, require_clean_exact_commit, verify_signs,
    wait_for_ports,
};
use super::{PicoArgs, PicoResult};

// The fixture has two bounded 20-second radio phases followed by distinct
// 30-second association, DHCP, and DNS phases plus bounded HTTP I/O. Keep the
// host deadline finite while allowing the firmware to emit its exact terminal
// failure at the end of any reviewed phase.
const PHYSICAL_TIMEOUT: Duration = Duration::from_secs(180);
const CLIENT_SERIAL_NEEDLE: &str = "conduit-pico-hil-client";

#[derive(Serialize)]
struct TwoPicoPhysicalRecord {
    schema: &'static str,
    #[serde(flatten)]
    proof: crate::proof::ProofRecord,
    appliance_firmware_build_id: String,
    appliance_firmware_sha256: String,
    appliance_hardware_identity: String,
    client_firmware_build_id: String,
    client_firmware_sha256: String,
    client_hardware_identity: String,
    leased_address: String,
    dns_name: &'static str,
    dns_address: [u8; 4],
    http_body: &'static str,
    appliance_runtime_boot_id: String,
    appliance_signs: Vec<serde_json::Value>,
    client_receipt: serde_json::Value,
    physical_acceptance: bool,
}

pub fn run_prove_pico_appliance_hil(
    appliance_link_port: Option<&str>,
    appliance_sign_port: Option<&str>,
    client_link_port: Option<&str>,
    client_sign_port: Option<&str>,
    dry_run: bool,
) -> PicoResult<()> {
    if dry_run {
        println!("==> Two-Pico appliance physical proof (dry-run)");
        println!("  build and flash fixture-only HIL client image on Pico B");
        println!("  retain its distinct USB identity while the probe waits for CDC DTR");
        println!("  build and flash exact appliance image on Pico A");
        println!("  open both CDC 1 ports without changing host networking");
        println!("  Pico B proves open association, DHCP, DNS, and literal HTTP response");
        println!("  correlate Pico B receipt with Pico A exact terminal Signs");
        println!("  proof class: physical-local-hardware");
        return Ok(());
    }

    let exact_commit = require_clean_exact_commit()?;
    let appliance_link_port = appliance_link_port
        .ok_or("two-Pico proof requires --link-port for the exact appliance Pico CDC 0")?;
    let appliance_sign_port = appliance_sign_port
        .ok_or("two-Pico proof requires --sign-port for the exact appliance Pico CDC 1")?;
    let client_link_port = client_link_port.ok_or(
        "two-Pico proof requires --client-link-port for the exact client Pico CDC 0 before flashing",
    )?;

    let client_args = PicoArgs {
        appliance_hil_client: true,
        link_port: Some(client_link_port.to_owned()),
        ..Default::default()
    };
    super::run_build(&client_args)?;
    let client_identity = read_appliance_hil_client_identity_manifest(&repo_root())?;
    require_identity_commit(&client_identity.git_revision, &exact_commit, "HIL client")?;
    snapshot_artifact("client", &client_identity)?;
    super::run_flash(&client_args)?;
    let (_, client_sign_path) = wait_for_client_ports(client_sign_port)?;

    let appliance_args = PicoArgs {
        appliance_hello: true,
        link_port: Some(appliance_link_port.to_owned()),
        port: Some(appliance_sign_port.to_owned()),
        ..Default::default()
    };
    super::run_build(&appliance_args)?;
    let appliance_identity = read_appliance_identity_manifest(&repo_root())?;
    require_identity_commit(&appliance_identity.git_revision, &exact_commit, "appliance")?;
    snapshot_artifact("appliance", &appliance_identity)?;
    super::run_flash(&appliance_args)?;
    let (_, appliance_sign_path) =
        wait_for_ports(Some(appliance_link_port), Some(appliance_sign_port))?;
    let appliance_sign_file = open_sign(&appliance_sign_path)?;
    let appliance_build_id = appliance_identity.firmware_build_id.clone();
    let appliance_reader = std::thread::spawn(move || {
        verify_signs(
            BufReader::new(appliance_sign_file),
            &appliance_build_id,
            None,
        )
        .map_err(|error| error.to_string())
    });
    // Start draining the appliance endpoint before asserting client DTR. A
    // fresh CDC open can block in the kernel long enough for the appliance's
    // mandatory first Sign to fill its endpoint and prevent service startup.
    let client_sign_file = open_sign(&client_sign_path)?;
    let client_receipt = read_client_receipt(
        BufReader::new(client_sign_file),
        &client_identity.firmware_build_id,
    )?;
    let leased_address = client_receipt["leased_address"]
        .as_str()
        .map(ToOwned::to_owned);
    let appliance_result = appliance_reader
        .join()
        .map_err(|_| "Pico appliance Sign reader panicked")?;
    let (appliance_runtime_boot_id, appliance_signs) = appliance_result
        .map_err(|error| format!("Pico appliance Sign verification failed: {error}"))?;
    verify_client_receipt(&client_receipt, &client_identity.firmware_build_id)?;
    let leased_address =
        leased_address.ok_or("successful two-Pico client receipt has no leased address")?;
    verify_appliance_lease_signs(&appliance_signs, &leased_address)?;

    let appliance_hardware_identity = hardware_identity(&appliance_sign_path)?;
    let client_hardware_identity = hardware_identity(&client_sign_path)?;
    let contract = crate::proof::CURRENT_PROOF_COMMANDS
        .iter()
        .find(|contract| contract.id == "pico.appliance-hello-two-pico-physical")
        .ok_or("two-Pico appliance proof command is absent from the proof catalog")?;
    let proof = crate::proof::ProofRecord {
        schema_version: crate::proof::PROOF_SCHEMA_VERSION,
        git_commit: exact_commit,
        dirty: false,
        proof_class: crate::proof::ProofClass::PhysicalLocalHardware,
        command: contract.command.into(),
        required_tools_or_targets: contract
            .required_tools_or_targets
            .iter()
            .map(ToString::to_string)
            .collect(),
        named_artifacts: contract
            .named_artifacts
            .iter()
            .map(ToString::to_string)
            .collect(),
        host_or_board_identity: Some(format!(
            "appliance={appliance_hardware_identity};client={client_hardware_identity}"
        )),
        success: true,
        timestamp: physical_timestamp()?,
        claims: contract
            .allowed_claims
            .iter()
            .map(ToString::to_string)
            .collect(),
    };
    proof
        .validate_against(contract)
        .map_err(|error| format!("two-Pico physical proof record is invalid: {error}"))?;
    let record = TwoPicoPhysicalRecord {
        schema: "conduit.pico-appliance/two-pico-physical-proof@1",
        proof,
        appliance_firmware_build_id: appliance_identity.firmware_build_id,
        appliance_firmware_sha256: appliance_identity.firmware_sha256,
        appliance_hardware_identity,
        client_firmware_build_id: client_identity.firmware_build_id,
        client_firmware_sha256: client_identity.firmware_sha256,
        client_hardware_identity,
        leased_address,
        dns_name: conduit_net::APPLIANCE_LOCAL_NAME,
        dns_address: conduit_net::DHCP_SERVER_ADDRESS,
        http_body: conduit_net::APPLIANCE_HELLO_BODY,
        appliance_runtime_boot_id,
        appliance_signs,
        client_receipt,
        physical_acceptance: true,
    };
    let destination = repo_root().join("target/pico-appliance-two-pico-physical.json");
    std::fs::write(&destination, serde_json::to_string_pretty(&record)?)?;
    println!(
        "==> Two-Pico appliance physical proof passed; record {}",
        destination.display()
    );
    Ok(())
}

fn require_identity_commit(actual: &str, expected: &str, role: &str) -> PicoResult<()> {
    if actual != expected {
        return Err(
            format!("{role} firmware identity is not bound to the exact clean commit").into(),
        );
    }
    Ok(())
}

fn snapshot_artifact(role: &str, identity: &impl Serialize) -> PicoResult<()> {
    let directory = repo_root().join("target/pico-appliance-hil");
    std::fs::create_dir_all(&directory)?;
    std::fs::copy(
        uf2_path(&repo_root()),
        directory.join(format!("{role}.uf2")),
    )?;
    std::fs::write(
        directory.join(format!("{role}.identity.json")),
        serde_json::to_string_pretty(identity)?,
    )?;
    Ok(())
}

fn open_sign(path: &Path) -> PicoResult<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    conduit_std_host::usb_cdc::configure_cdc_port(&file, 0, 100).map_err(|error| {
        format!(
            "failed configuring Pico appliance sign port {}: {error}",
            path.display()
        )
    })?;
    conduit_std_host::usb_cdc::assert_dtr(&file).map_err(|error| {
        format!(
            "failed asserting DTR on Pico appliance sign port {}: {error}",
            path.display()
        )
    })?;
    Ok(file)
}

fn wait_for_client_ports(explicit_sign: Option<&str>) -> PicoResult<(PathBuf, PathBuf)> {
    let deadline = Instant::now() + PHYSICAL_TIMEOUT;
    loop {
        if let Some(sign) = explicit_sign {
            let sign = PathBuf::from(sign);
            if sign.exists() {
                return Ok((PathBuf::new(), sign));
            }
        } else if let Ok(ports) = discover_client_ports() {
            return Ok(ports);
        }
        if Instant::now() >= deadline {
            return Err(
                "timed out waiting for the distinct appliance HIL client USB identity".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn discover_client_ports() -> PicoResult<(PathBuf, PathBuf)> {
    let directory = Path::new("/dev/serial/by-id");
    let mut link = None;
    let mut sign = None;
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !name.contains(CLIENT_SERIAL_NEEDLE) {
            continue;
        }
        if name.ends_with("-if00") {
            link = Some(path);
        } else if name.ends_with("-if02") {
            sign = Some(path);
        }
    }
    match (link, sign) {
        (Some(link), Some(sign)) => Ok((link, sign)),
        _ => Err("distinct appliance HIL client CDC pair is incomplete".into()),
    }
}

fn read_client_receipt(
    mut reader: impl BufRead,
    firmware_build_id: &str,
) -> PicoResult<serde_json::Value> {
    let deadline = Instant::now() + PHYSICAL_TIMEOUT;
    let mut line = String::new();
    loop {
        if !read_complete_bounded_line(&mut reader, &mut line, "appliance HIL client receipt")? {
            if Instant::now() >= deadline {
                return Err("timed out waiting for appliance HIL client receipt".into());
            }
            continue;
        }
        let receipt: serde_json::Value = serde_json::from_str(line.trim())?;
        verify_client_identity(&receipt, firmware_build_id)?;
        return Ok(receipt);
    }
}

fn verify_client_identity(receipt: &serde_json::Value, firmware_build_id: &str) -> PicoResult<()> {
    let exact = receipt["schema"].as_str() == Some("conduit.pico-appliance/hil-client@1")
        && receipt["firmware_build_id"].as_str() == Some(firmware_build_id)
        && receipt["host_id"].as_str() == Some("pico/appliance-hil-client")
        && receipt["runtime_boot_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
        && receipt["ssid"].as_str() == Some(conduit_net::APPLIANCE_SSID)
        && receipt["terminal"].as_bool() == Some(true);
    if !exact {
        return Err(format!("Pico appliance HIL client identity is not exact: {receipt}").into());
    }
    Ok(())
}

fn verify_client_receipt(receipt: &serde_json::Value, firmware_build_id: &str) -> PicoResult<()> {
    verify_client_identity(receipt, firmware_build_id)?;
    let exact = receipt["success"].as_bool() == Some(true)
        && receipt.get("failure").is_none()
        && receipt["dns_name"].as_str() == Some(conduit_net::APPLIANCE_LOCAL_NAME)
        && receipt["dns_address"].as_str() == Some("192.168.4.1")
        && receipt["http_body"].as_str() == Some(conduit_net::APPLIANCE_HELLO_BODY);
    let lease = receipt["leased_address"]
        .as_str()
        .and_then(|address| address.rsplit_once('.'))
        .and_then(|(prefix, host)| host.parse::<u8>().ok().map(|host| (prefix, host)));
    if !exact || !matches!(lease, Some(("192.168.4", 2..=5))) {
        return Err(format!("Pico appliance HIL client receipt is not exact: {receipt}").into());
    }
    Ok(())
}

fn verify_appliance_lease_signs(
    signs: &[serde_json::Value],
    leased_address: &str,
) -> PicoResult<()> {
    for index in [1, 2] {
        if signs.get(index).and_then(|sign| sign["address"].as_str()) != Some(leased_address) {
            return Err("Pico appliance association/lease Sign address mismatch".into());
        }
    }
    Ok(())
}

fn hardware_identity(path: &Path) -> PicoResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Pico sign port has no hardware identity".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.pico-appliance/hil-client@1",
            "firmware_build_id": "client-build",
            "host_id": "pico/appliance-hil-client",
            "runtime_boot_id": "client-runtime/1",
            "ssid": conduit_net::APPLIANCE_SSID,
            "terminal": true,
            "success": true,
            "leased_address": "192.168.4.2",
            "dns_name": conduit_net::APPLIANCE_LOCAL_NAME,
            "dns_address": "192.168.4.1",
            "http_body": conduit_net::APPLIANCE_HELLO_BODY,
        })
    }

    #[test]
    fn client_receipt_requires_exact_physical_results() {
        verify_client_receipt(&receipt(), "client-build").unwrap();

        let mut failure = receipt();
        failure["success"] = false.into();
        failure["failure"] = "http-response-mismatch".into();
        assert!(verify_client_receipt(&failure, "client-build").is_err());

        let mut wrong_lease = receipt();
        wrong_lease["leased_address"] = "192.168.4.9".into();
        assert!(verify_client_receipt(&wrong_lease, "client-build").is_err());
    }
}
