use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{cli::GlobalOpts, workspace::workspace_root};

const SCHEMA: &str = "conduit.host/esp32-physical-inspection@2";
const ESPTOOL_VERSION: &str = "5.3.0";
const ESPTOOL_ARCHIVE: &str = "esptool-v5.3.0-linux-amd64.tar.gz";
const ESPTOOL_URL: &str =
    "https://github.com/espressif/esptool/releases/download/v5.3.0/esptool-v5.3.0-linux-amd64.tar.gz";
const ESPTOOL_ARCHIVE_SHA256: &str =
    "46ca7b52c309790bc4d140990680f6088e8cad40b230fda6999661efc24845b6";
const ESPTOOL_BINARY_SHA256: &str =
    "7a2862c7c5d0467f2cd8e4c0b0dc8ae9b59e065489027db63875a19e21ea7122";
const MAX_FIELD_BYTES: usize = 512;
const MAX_OUTPUT_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, clap::ValueEnum, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Esp32SocClass {
    ClassicEsp32,
    Esp32C3,
    Esp32S3,
}

impl Esp32SocClass {
    fn accepts_chip(self, chip: &str) -> bool {
        match self {
            Self::ClassicEsp32 => chip == "ESP32-D0WD-V3",
            Self::Esp32C3 => chip == "ESP32-C3" || chip.starts_with("ESP32-C3 "),
            Self::Esp32S3 => chip == "ESP32-S3" || chip.starts_with("ESP32-S3 "),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct InspectionReceipt {
    schema: &'static str,
    proof_class: &'static str,
    status: &'static str,
    git_head: String,
    expected_soc_class: Esp32SocClass,
    physical_markings: PhysicalMarkings,
    serial_base: SerialBase,
    rom: RomFacts,
    flash: FlashFacts,
    security_info: SecurityInfo,
    tool: ToolIdentity,
    claims_excluded: Vec<&'static str>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct PhysicalMarkings {
    board: String,
    module: String,
    board_revision: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct SerialBase {
    requested_path: String,
    canonical_device: String,
    usb_vendor_id: String,
    usb_product_id: String,
    usb_serial: String,
    usb_driver: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct RomFacts {
    pub(super) chip: String,
    pub(super) revision: String,
    pub(super) features: Vec<String>,
    pub(super) crystal_mhz: u32,
    pub(super) mac: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct FlashFacts {
    pub(super) manufacturer_id: String,
    pub(super) device_id: String,
    pub(super) detected_bytes: u64,
    pub(super) voltage: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum SecurityInfo {
    Unsupported { detail: String },
    Observed { detail: String },
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ToolIdentity {
    name: &'static str,
    version: &'static str,
    archive_sha256: &'static str,
    binary_sha256: &'static str,
}

pub(super) fn run(
    port: &Path,
    expected_soc: Esp32SocClass,
    board_marking: &str,
    module_marking: &str,
    board_revision: &str,
    output: &Path,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    for (name, value) in [
        ("board marking", board_marking),
        ("module marking", module_marking),
        ("board revision", board_revision),
    ] {
        validate_field(name, value)?;
    }
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would inspect ESP32 at {} and write {} without modifying flash",
                port.display(),
                output.display()
            );
        }
        return Ok(());
    }
    if !port.exists() {
        return Err(format!("ESP32 serial path does not exist: {}", port.display()).into());
    }
    let root = workspace_root()?;
    let tool = provision_esptool(&root)?;
    let udev = command_output(
        "udevadm",
        &["info", "--query=property", "--name"],
        Some(port),
    )?;
    let properties = parse_properties(&udev);
    let chip = run_esptool(&tool, port, "chip-id")?;
    let flash = run_esptool(&tool, port, "flash-id")?;
    let security = run_esptool_allow_failure(&tool, port, "get-security-info")?;
    let receipt = InspectionReceipt {
        schema: SCHEMA,
        proof_class: "physical-inspection",
        status: "observed",
        git_head: git_head(&root)?,
        expected_soc_class: expected_soc,
        physical_markings: PhysicalMarkings {
            board: board_marking.to_owned(),
            module: module_marking.to_owned(),
            board_revision: board_revision.to_owned(),
        },
        serial_base: serial_base(port, &properties)?,
        rom: parse_rom_facts(&chip, expected_soc)?,
        flash: parse_flash_facts(&flash)?,
        security_info: parse_security_info(&security)?,
        tool: ToolIdentity {
            name: "esptool",
            version: ESPTOOL_VERSION,
            archive_sha256: ESPTOOL_ARCHIVE_SHA256,
            binary_sha256: ESPTOOL_BINARY_SHA256,
        },
        claims_excluded: vec![
            "host",
            "boot",
            "host-offer",
            "image",
            "firmware-execution",
            "peripheral-availability",
            "line-readiness",
        ],
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_vec_pretty(&receipt)?)?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!(
            "INSPECTED {} {} with {} bytes of SPI flash",
            receipt.rom.chip, receipt.rom.revision, receipt.flash.detected_bytes
        );
        println!("evidence: {}", output.display());
    }
    Ok(())
}

fn provision_esptool(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let directory = root.join("target/esp32-tools");
    let archive = directory.join(ESPTOOL_ARCHIVE);
    let extracted = directory.join(format!("esptool-{ESPTOOL_VERSION}"));
    let binary = extracted.join("esptool");
    if binary.is_file() && sha256_file(&binary)? == ESPTOOL_BINARY_SHA256 {
        return Ok(binary);
    }
    fs::create_dir_all(&directory)?;
    let status = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error"])
        .arg(ESPTOOL_URL)
        .arg("--output")
        .arg(&archive)
        .status()?;
    if !status.success() {
        return Err("failed to download pinned esptool archive".into());
    }
    let found = sha256_file(&archive)?;
    if found != ESPTOOL_ARCHIVE_SHA256 {
        return Err(format!(
            "esptool archive digest mismatch: expected {ESPTOOL_ARCHIVE_SHA256}, found {found}"
        )
        .into());
    }
    fs::create_dir_all(&extracted)?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(&extracted)
        .args(["--strip-components=1"])
        .status()?;
    if !status.success() || !binary.is_file() {
        return Err("failed to extract pinned esptool archive".into());
    }
    let found = sha256_file(&binary)?;
    if found != ESPTOOL_BINARY_SHA256 {
        return Err(format!(
            "esptool binary digest mismatch: expected {ESPTOOL_BINARY_SHA256}, found {found}"
        )
        .into());
    }
    Ok(binary)
}

fn run_esptool(
    tool: &Path,
    port: &Path,
    operation: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let output = esptool_output(tool, port, operation)?;
    if !output.status.success() {
        return Err(format!("esptool {operation} failed: {}", bounded_output(&output)).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn run_esptool_allow_failure(
    tool: &Path,
    port: &Path,
    operation: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    esptool_output(tool, port, operation)
}

fn esptool_output(
    tool: &Path,
    port: &Path,
    operation: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(tool)
        .arg("--port")
        .arg(port)
        .arg(operation)
        .output()?)
}

fn command_output(
    program: &str,
    args: &[&str],
    path: Option<&Path>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(path) = path {
        command.arg(path);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("{program} failed: {}", bounded_output(&output)).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn parse_properties(source: &str) -> BTreeMap<String, String> {
    source
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn serial_base(
    port: &Path,
    properties: &BTreeMap<String, String>,
) -> Result<SerialBase, Box<dyn std::error::Error>> {
    Ok(SerialBase {
        requested_path: port.display().to_string(),
        canonical_device: fs::canonicalize(port)?.display().to_string(),
        usb_vendor_id: property(properties, "ID_VENDOR_ID")?,
        usb_product_id: property(properties, "ID_MODEL_ID")?,
        usb_serial: property(properties, "ID_SERIAL_SHORT")?,
        usb_driver: property(properties, "ID_USB_DRIVER")?,
    })
}

pub(super) fn parse_rom_facts(
    source: &str,
    expected_soc: Esp32SocClass,
) -> Result<RomFacts, Box<dyn std::error::Error>> {
    let chip_line = line_value(source, "Chip type:")?;
    let (chip, revision) = chip_line
        .rsplit_once(" (revision ")
        .and_then(|(chip, revision)| revision.strip_suffix(')').map(|revision| (chip, revision)))
        .ok_or("malformed ESP32 chip type/revision")?;
    let features = line_value(source, "Features:")?
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !expected_soc.accepts_chip(chip) {
        return Err(format!(
            "ESP32 SoC-class mismatch: expected {expected_soc:?}, observed {chip}"
        )
        .into());
    }
    let crystal_mhz = line_value(source, "Crystal frequency:")?
        .strip_suffix("MHz")
        .ok_or("malformed crystal frequency")?
        .trim()
        .parse()?;
    let mac = line_value(source, "MAC:")?;
    validate_mac(mac)?;
    Ok(RomFacts {
        chip: chip.to_owned(),
        revision: revision.to_owned(),
        features,
        crystal_mhz,
        mac: mac.to_owned(),
    })
}

pub(super) fn parse_flash_facts(source: &str) -> Result<FlashFacts, Box<dyn std::error::Error>> {
    let size = line_value(source, "Detected flash size:")?;
    let detected_bytes = size
        .strip_suffix("MB")
        .ok_or("unsupported flash-size unit")?
        .trim()
        .parse::<u64>()?
        .checked_mul(1024 * 1024)
        .ok_or("flash size overflow")?;
    let strapped_voltage = optional_line_value(source, "Flash voltage set by a strapping pin:");
    let efuse_voltage = optional_line_value(source, "Flash voltage set by eFuse:");
    let voltage = match (strapped_voltage, efuse_voltage) {
        (Some(voltage), None) | (None, Some(voltage)) => Some(voltage.to_owned()),
        // Parts with embedded flash may omit a separate voltage from
        // `flash-id`; retain the absence instead of inventing a value.
        (None, None) => None,
        (Some(_), Some(_)) => return Err("ambiguous flash voltage sources".into()),
    };
    Ok(FlashFacts {
        manufacturer_id: normalize_hex(line_value(source, "Manufacturer:")?)?,
        device_id: normalize_hex(line_value(source, "Device:")?)?,
        detected_bytes,
        voltage,
    })
}

fn parse_security_info(output: &Output) -> Result<SecurityInfo, Box<dyn std::error::Error>> {
    let detail = bounded_output(output);
    if output.status.success() {
        Ok(SecurityInfo::Observed { detail })
    } else if detail.contains("Command not implemented") {
        Ok(SecurityInfo::Unsupported { detail })
    } else {
        Err(format!("security inspection failed unexpectedly: {detail}").into())
    }
}

fn line_value<'a>(source: &'a str, prefix: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    optional_line_value(source, prefix)
        .ok_or_else(|| format!("missing inspection field {prefix}").into())
}

fn optional_line_value<'a>(source: &'a str, prefix: &str) -> Option<&'a str> {
    source
        .lines()
        .find_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
        .filter(|value| !value.is_empty())
}

fn normalize_hex(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("malformed hexadecimal value {value:?}").into());
    }
    Ok(format!("0x{}", value.to_ascii_lowercase()))
}

fn validate_mac(value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 6
        || fields
            .iter()
            .any(|field| field.len() != 2 || !field.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(format!("malformed MAC address {value:?}").into());
    }
    Ok(())
}

fn validate_field(name: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.trim().is_empty()
        || value.len() > MAX_FIELD_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!("invalid {name}").into());
    }
    Ok(())
}

fn property(
    properties: &BTreeMap<String, String>,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    properties
        .get(key)
        .filter(|value| !value.is_empty() && value.len() <= MAX_FIELD_BYTES)
        .cloned()
        .ok_or_else(|| format!("missing udev property {key}").into())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn git_head(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("cannot resolve exact git head".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn bounded_output(output: &Output) -> String {
    let mut value = String::from_utf8_lossy(&output.stdout).into_owned();
    value.push_str(&String::from_utf8_lossy(&output.stderr));
    value.truncate(value.floor_char_boundary(MAX_OUTPUT_BYTES));
    value
}
