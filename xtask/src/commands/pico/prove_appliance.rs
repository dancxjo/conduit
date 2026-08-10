//! Exact physical client proof for the finite Pico W Hello appliance.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddrV4, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::appliance_identity::read_appliance_identity_manifest;
use super::doctor::repo_root;
use super::serial::resolve_dual_ports;
use super::{PicoArgs, PicoResult};

const PHYSICAL_TIMEOUT: Duration = Duration::from_secs(30);
const EXPECTED_SIGNS: [&str; 8] = [
    "ap-ready",
    "client-associated",
    "dhcp-lease",
    "dns-request",
    "dns-response",
    "http-request",
    "http-response",
    "terminal",
];

#[derive(Serialize)]
struct PhysicalApplianceRecord {
    schema: &'static str,
    #[serde(flatten)]
    proof: crate::proof::ProofRecord,
    firmware_build_id: String,
    firmware_sha256: String,
    hardware_identity: String,
    client_interface: String,
    leased_address: String,
    dns_name: &'static str,
    dns_address: [u8; 4],
    http_body: &'static str,
    runtime_boot_id: String,
    signs: Vec<serde_json::Value>,
    physical_acceptance: bool,
}

pub fn run_prove_pico_appliance(
    link_port: Option<&str>,
    sign_port: Option<&str>,
    client_interface: Option<&str>,
    args: &PicoArgs,
) -> PicoResult<()> {
    if args.dry_run {
        println!("==> Pico appliance physical proof (dry-run)");
        println!("  build exact --appliance-hello image and identity");
        println!("  flash exact UF2 through cargo xtask Pico workflow");
        println!(
            "  sign port: {}",
            sign_port.unwrap_or("<auto-discover exact Pico CDC 1>")
        );
        println!(
            "  client interface: {}",
            client_interface.unwrap_or("<require exactly one NetworkManager Wi-Fi device>")
        );
        println!("  join open SSID: {}", conduit_net::APPLIANCE_SSID);
        println!("  require bounded DHCP, DNS, HTTP, and exact terminal Signs");
        println!("  safety: refuse an interface with an active connection");
        println!("  proof class: physical-local-hardware");
        return Ok(());
    }

    let interface = resolve_wifi_interface(client_interface)?;
    let previous_connection = active_connection(&interface)?;
    require_dedicated_wifi_interface(&interface, previous_connection.as_deref())?;
    super::run_build(args)?;
    let exact_commit = require_clean_exact_commit()?;
    super::run_flash(args)?;
    let identity = read_appliance_identity_manifest(&repo_root())?;
    if identity.git_revision != exact_commit {
        return Err("Pico appliance identity is not bound to the exact clean commit".into());
    }
    let (_, sign_path) = wait_for_ports(link_port, sign_port)?;
    let hardware_identity = sign_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Pico sign port has no stable hardware identity")?
        .to_owned();
    let sign_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&sign_path)?;
    conduit_std_host::usb_cdc::configure_cdc_port(&sign_file, 0, 100).map_err(|error| {
        format!(
            "failed configuring Pico appliance sign port {}: {error}",
            sign_path.display()
        )
    })?;
    conduit_std_host::usb_cdc::assert_dtr(&sign_file).map_err(|error| {
        format!(
            "failed asserting DTR on Pico appliance sign port {}: {error}",
            sign_path.display()
        )
    })?;

    let restoration = WifiRestoration::new(interface.clone(), previous_connection);
    wait_for_ssid(&interface)?;
    run_nmcli(&[
        "--wait",
        "20",
        "device",
        "wifi",
        "connect",
        conduit_net::APPLIANCE_SSID,
        "ifname",
        &interface,
    ])?;
    let leased_address = leased_address(&interface)?;
    let dns_address = prove_dns()?;
    prove_http()?;
    let (runtime_boot_id, signs) = verify_signs(
        BufReader::new(sign_file),
        &identity.firmware_build_id,
        &leased_address,
    )?;
    restoration.restore()?;

    let contract = crate::proof::CURRENT_PROOF_COMMANDS
        .iter()
        .find(|contract| contract.id == "pico.appliance-hello-physical")
        .ok_or("Pico appliance proof command is absent from the proof catalog")?;
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
        host_or_board_identity: Some(hardware_identity.clone()),
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
        .map_err(|error| format!("physical proof record is invalid: {error}"))?;
    let record = PhysicalApplianceRecord {
        schema: "conduit.pico-appliance/physical-proof@1",
        proof,
        firmware_build_id: identity.firmware_build_id.clone(),
        firmware_sha256: identity.firmware_sha256.clone(),
        hardware_identity,
        client_interface: interface,
        leased_address,
        dns_name: conduit_net::APPLIANCE_LOCAL_NAME,
        dns_address,
        http_body: conduit_net::APPLIANCE_HELLO_BODY,
        runtime_boot_id,
        signs,
        physical_acceptance: true,
    };
    let destination = repo_root().join("target/pico-appliance-physical.json");
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&destination, serde_json::to_string_pretty(&record)?)?;
    println!(
        "==> Pico appliance physical proof passed; record {}",
        destination.display()
    );
    Ok(())
}

pub(super) fn require_clean_exact_commit() -> PicoResult<String> {
    let root = repo_root();
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err("physical Pico appliance acceptance requires a clean exact commit".into());
    }
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !commit.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    Ok(String::from_utf8(commit.stdout)?.trim().to_owned())
}

pub(super) fn physical_timestamp() -> PicoResult<String> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    Ok(format!("unix:{seconds}"))
}

pub(super) fn wait_for_ports(
    link: Option<&str>,
    sign: Option<&str>,
) -> PicoResult<(PathBuf, PathBuf)> {
    let deadline = Instant::now() + PHYSICAL_TIMEOUT;
    loop {
        match resolve_dual_ports(link, sign) {
            Ok(paths) if paths.0.exists() && paths.1.exists() => return Ok(paths),
            Ok(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Ok(paths) => {
                return Err(format!(
                    "timed out waiting for Pico CDC paths {} and {} to become usable",
                    paths.0.display(),
                    paths.1.display()
                )
                .into())
            }
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => return Err(error),
        }
    }
}

fn resolve_wifi_interface(requested: Option<&str>) -> PicoResult<String> {
    if let Some(interface) = requested {
        return Ok(interface.to_owned());
    }
    let output = nmcli_output(&["-t", "-f", "DEVICE,TYPE", "device", "status"])?;
    let devices = output
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter_map(|(device, kind)| (kind == "wifi").then_some(device.to_owned()))
        .collect::<Vec<_>>();
    match devices.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err("physical Pico appliance proof found no NetworkManager Wi-Fi device".into()),
        _ => Err("multiple Wi-Fi devices found; pass --client-interface".into()),
    }
}

fn wait_for_ssid(interface: &str) -> PicoResult<()> {
    let deadline = Instant::now() + PHYSICAL_TIMEOUT;
    loop {
        let _ = run_nmcli(&["device", "wifi", "rescan", "ifname", interface]);
        let output = nmcli_output(&[
            "-t", "-f", "SSID", "device", "wifi", "list", "ifname", interface,
        ])?;
        if output
            .lines()
            .any(|ssid| ssid == conduit_net::APPLIANCE_SSID)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for Pico appliance SSID `{}`",
                conduit_net::APPLIANCE_SSID
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn active_connection(interface: &str) -> PicoResult<Option<String>> {
    let output = nmcli_output(&["-g", "GENERAL.CONNECTION", "device", "show", interface])?;
    let value = output.trim();
    Ok((!value.is_empty() && value != "--").then(|| value.to_owned()))
}

fn require_dedicated_wifi_interface(
    interface: &str,
    active_connection: Option<&str>,
) -> PicoResult<()> {
    if let Some(connection) = active_connection {
        return Err(format!(
            "refusing to move active Wi-Fi interface `{interface}` from connection `{connection}`; use a disconnected dedicated client interface so the proof cannot sever this host's network path"
        )
        .into());
    }
    Ok(())
}

fn leased_address(interface: &str) -> PicoResult<String> {
    let output = nmcli_output(&["-g", "IP4.ADDRESS", "device", "show", interface])?;
    let address = output
        .lines()
        .next()
        .and_then(|line| line.split('/').next())
        .ok_or("Pico appliance client received no IPv4 lease")?;
    let octets = parse_ipv4(address)?;
    if octets[..3] != [192, 168, 4] || !(2..=5).contains(&octets[3]) {
        return Err(format!("Pico appliance returned out-of-pool lease {address}").into());
    }
    Ok(address.to_owned())
}

fn prove_dns() -> PicoResult<[u8; 4]> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut query = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let query_len = conduit_net::encode_appliance_dns_query(0xc011, &mut query)
        .map_err(|error| format!("failed building exact DNS query: {error:?}"))?;
    socket.send_to(&query[..query_len], "192.168.4.1:53")?;
    let mut actual = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let (actual_len, _) = socket.recv_from(&mut actual)?;
    let mut expected = [0; conduit_net::MAXIMUM_DNS_PACKET_BYTES as usize];
    let expected_len = conduit_net::answer_appliance_dns(&query[..query_len], &mut expected)
        .map_err(|error| format!("failed building expected DNS answer: {error:?}"))?;
    if actual[..actual_len] != expected[..expected_len] {
        return Err("physical Pico returned a non-exact DNS answer".into());
    }
    Ok(conduit_net::DHCP_SERVER_ADDRESS)
}

fn prove_http() -> PicoResult<()> {
    let mut stream = TcpStream::connect_timeout(
        &SocketAddrV4::new(conduit_net::DHCP_SERVER_ADDRESS.into(), 80).into(),
        Duration::from_secs(5),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(b"GET / HTTP/1.1\r\nHost: hello.conduit\r\nConnection: close\r\n\r\n")?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    if response != conduit_net::HTTP_HELLO_RESPONSE {
        return Err("physical Pico returned a non-exact HTTP Hello response".into());
    }
    Ok(())
}

pub(super) fn verify_signs(
    mut reader: impl BufRead,
    firmware_build_id: &str,
    leased_address: &str,
) -> PicoResult<(String, Vec<serde_json::Value>)> {
    let deadline = Instant::now() + PHYSICAL_TIMEOUT;
    let mut line = String::new();
    let mut signs = Vec::new();
    let mut runtime_boot = None;
    while signs.len() < EXPECTED_SIGNS.len() && Instant::now() < deadline {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let sign: serde_json::Value = serde_json::from_str(line.trim())?;
        let index = signs.len();
        verify_field(&sign, "schema", "conduit.pico-appliance/sign@1")?;
        verify_field(&sign, "firmware_build_id", firmware_build_id)?;
        verify_field(&sign, "profile", conduit_net::PICO_APPLIANCE_PROFILE)?;
        verify_field(&sign, "host_id", "pico/appliance-hello")?;
        verify_field(&sign, "kind", EXPECTED_SIGNS[index])?;
        if sign["sequence"].as_u64() != Some(index as u64 + 1) {
            return Err("Pico appliance Sign sequence is not exact".into());
        }
        let current_boot = sign["runtime_boot_id"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("Pico appliance Sign has no runtime boot identity")?;
        match &runtime_boot {
            Some(expected) if expected != current_boot => {
                return Err("Pico appliance runtime boot identity changed".into())
            }
            None => runtime_boot = Some(current_boot.to_owned()),
            _ => {}
        }
        let expected_sign_id = format!("pico/appliance/sign:{current_boot}:{:02}", index + 1);
        verify_field(&sign, "sign_id", &expected_sign_id)?;
        if sign.get("failure").is_some() {
            return Err(format!("Pico appliance emitted failure Sign: {sign}").into());
        }
        if matches!(index, 1 | 2) && sign["address"].as_str() != Some(leased_address) {
            return Err("Pico appliance association/lease Sign address mismatch".into());
        }
        signs.push(sign);
    }
    if signs.len() != EXPECTED_SIGNS.len() {
        return Err(format!(
            "expected {} exact Pico appliance Signs, received {}",
            EXPECTED_SIGNS.len(),
            signs.len()
        )
        .into());
    }
    Ok((runtime_boot.unwrap(), signs))
}

fn verify_field(record: &serde_json::Value, field: &str, expected: &str) -> PicoResult<()> {
    if record[field].as_str() != Some(expected) {
        return Err(format!("Pico appliance Sign field `{field}` mismatch").into());
    }
    Ok(())
}

fn parse_ipv4(value: &str) -> PicoResult<[u8; 4]> {
    let octets = value
        .split('.')
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>()?;
    octets
        .try_into()
        .map_err(|_| format!("invalid IPv4 address `{value}`").into())
}

fn nmcli_output(args: &[&str]) -> PicoResult<String> {
    let output = Command::new("nmcli").args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "nmcli {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn run_nmcli(args: &[&str]) -> PicoResult<()> {
    nmcli_output(args).map(|_| ())
}

struct WifiRestoration {
    interface: String,
    previous: Option<String>,
    restored: bool,
}

impl WifiRestoration {
    fn new(interface: String, previous: Option<String>) -> Self {
        Self {
            interface,
            previous,
            restored: false,
        }
    }

    fn restore(mut self) -> PicoResult<()> {
        restore_wifi(&self.interface, self.previous.as_deref())?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for WifiRestoration {
    fn drop(&mut self) {
        if !self.restored {
            let _ = restore_wifi(&self.interface, self.previous.as_deref());
        }
    }
}

fn restore_wifi(interface: &str, previous: Option<&str>) -> PicoResult<()> {
    let _ = run_nmcli(&["connection", "delete", "id", conduit_net::APPLIANCE_SSID]);
    if let Some(connection) = previous {
        run_nmcli(&["connection", "up", "id", connection, "ifname", interface])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn physical_sign_contract_is_exact_and_bounded() {
        assert_eq!(EXPECTED_SIGNS.len(), 8);
        assert!(EXPECTED_SIGNS.len() <= conduit_net::MAXIMUM_APPLIANCE_SIGNS as usize);
        assert_eq!(EXPECTED_SIGNS[0], "ap-ready");
        assert_eq!(EXPECTED_SIGNS[7], "terminal");
        assert!(!EXPECTED_SIGNS.contains(&"success"));
    }

    #[test]
    fn lease_parser_accepts_only_reviewed_pool() {
        assert_eq!(parse_ipv4("192.168.4.2").unwrap(), [192, 168, 4, 2]);
        assert!(parse_ipv4("192.168.4").is_err());
    }

    #[test]
    fn physical_proof_refuses_an_active_wifi_connection() {
        assert!(require_dedicated_wifi_interface("wlan-proof", None).is_ok());
        let error = require_dedicated_wifi_interface("wlo1", Some("remote-access"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("refusing to move active Wi-Fi interface `wlo1`"));
        assert!(error.contains("disconnected dedicated client interface"));
    }

    fn transcript(mut mutate: impl FnMut(usize, &mut serde_json::Value)) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (index, kind) in EXPECTED_SIGNS.into_iter().enumerate() {
            let mut sign = serde_json::json!({
                "schema": "conduit.pico-appliance/sign@1",
                "firmware_build_id": "build/1",
                "profile": conduit_net::PICO_APPLIANCE_PROFILE,
                "host_id": "pico/appliance-hello",
                "runtime_boot_id": "runtime/boot/1",
                "sequence": index + 1,
                "sign_id": format!("pico/appliance/sign:runtime/boot/1:{:02}", index + 1),
                "kind": kind,
            });
            if matches!(index, 1 | 2) {
                sign["address"] = "192.168.4.2".into();
            }
            mutate(index, &mut sign);
            bytes.extend_from_slice(serde_json::to_string(&sign).unwrap().as_bytes());
            bytes.push(b'\n');
        }
        bytes
    }

    #[test]
    fn transcript_requires_exact_identity_order_lease_and_terminal() {
        let valid = transcript(|_, _| {});
        let (boot, signs) = verify_signs(Cursor::new(valid), "build/1", "192.168.4.2").unwrap();
        assert_eq!(boot, "runtime/boot/1");
        assert_eq!(signs.len(), EXPECTED_SIGNS.len());

        let wrong_kind = transcript(|index, sign| {
            if index == 4 {
                sign["kind"] = "http-response".into();
            }
        });
        assert!(verify_signs(Cursor::new(wrong_kind), "build/1", "192.168.4.2").is_err());

        let stale_build = transcript(|index, sign| {
            if index == 6 {
                sign["firmware_build_id"] = "stale".into();
            }
        });
        assert!(verify_signs(Cursor::new(stale_build), "build/1", "192.168.4.2").is_err());

        let wrong_lease = transcript(|_, _| {});
        assert!(verify_signs(Cursor::new(wrong_lease), "build/1", "192.168.4.3").is_err());
    }
}
