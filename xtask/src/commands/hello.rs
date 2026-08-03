use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HELLO_CRATE_MANIFEST: &str = "firmware/conduit-pico-w-hello/Cargo.toml";
const HELLO_TARGET: &str = "thumbv6m-none-eabi";
const HELLO_PROFILE: &str = "release";
const HELLO_PACKAGE: &str = "conduit-pico-w-hello";
const HELLO_UF2_FILE: &str = "conduit-pico-w-hello.uf2";
const HELLO_MANIFEST_FILE: &str = "conduit-pico-w-hello.identity.json";
const HELLO_STATUS_URL: &str = "http://192.168.4.1/status.json";
const HELLO_VERIFY_TIMEOUT_SECONDS: u64 = 90;
const HELLO_STATUS_TIMEOUT_SECONDS: u64 = 4;
const HELLO_POLL_INTERVAL_MS: u64 = 500;
const HELLO_STATUS_RETRY_MS: u64 = 1_000;
const CYW43_DIR: &str = "firmware/conduit-pico-w-hello/cyw43";
const CYW43_FIRMWARE_REF: &str = "6a823b96b3d270b6da1cc667f8acea749e588dab";
const CYW43_FILE_HASHES: [BlobSpec<'_>; 4] = [
    BlobSpec {
        name: "43439A0.bin",
        sha256: "5555e0261da2610a500d68c18d895cace0152bbefbf76f4aa683ebce77e3d7eb",
    },
    BlobSpec {
        name: "43439A0_clm.bin",
        sha256: "e712b3d218e8b1e2747b092e03b8b0afcb8c8c8e355d2a4a0d47b493800f3f89",
    },
    BlobSpec {
        name: "nvram_rp2040.bin",
        sha256: "4904bdbb0c937bd0ac2eb2a1d62f2da4dd90e32082384e02874e8d671b0f330d",
    },
    BlobSpec {
        name: "LICENSE-permissive-binary-license-1.0.txt",
        sha256:
            "5f65b8a496ac27afda41917c18cb6e690b4a022df1f5a12ea823eb38a287f50e",
    },
];

#[derive(Debug)]
pub struct HelloOptions {
    pub build_only: bool,
    pub mount: Option<PathBuf>,
    pub port: Option<String>,
    pub verify: bool,
    pub dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct IdentityManifest {
    #[serde(rename = "conduit_revision")]
    conduit_revision: String,
    #[serde(rename = "target")]
    target: String,
    #[serde(rename = "profile")]
    profile: String,
    #[serde(rename = "full_plan_hash")]
    full_plan_hash: String,
    #[serde(rename = "firmware_identity")]
    firmware_identity: String,
}

#[derive(Debug, Clone, Copy)]
struct BlobSpec<'a> {
    name: &'a str,
    sha256: &'a str,
}

pub fn run(workspace_root: &Path, options: HelloOptions) -> Result<()> {
    if options.dry_run {
        println!("DRY-RUN: would execute hello workflow");
        return Ok(());
    }

    println!("Checking host prerequisites...");
    check_prerequisites()?;

    if !cyw43_artifacts_present(workspace_root) {
        println!("CYW43 blobs missing or mismatched; fetching from pinned firmware commit {CYW43_FIRMWARE_REF}");
        fetch_cyw43_firmware(workspace_root)?;
    } else {
        println!("CYW43 blobs are present and pass pinned checks");
    }

    println!("Building firmware...");
    build_firmware(workspace_root)?;
    println!("Converting ELF -> UF2...");
    convert_uf2(workspace_root)?;

    let manifest = manifest(workspace_root)?;
    print_identity(&manifest);

    let mounted = if !options.build_only {
        Some(flash_firmware(
            workspace_root,
            options.mount.as_deref(),
            options.port.as_deref(),
        )?)
    } else {
        None
    };

    if options.verify || !options.build_only {
        let verify = try_verify_identity(&manifest);
        match verify {
            Ok(()) => println!("verified: running firmware matches built identity"),
            Err(error) => {
                if options.verify {
                    return Err(error);
                }
                println!("skipped identity verification: {error}");
            }
        }
    }

    print_connection_instructions(
        workspace_root,
        &manifest,
        mounted.as_deref(),
        &options.port,
    );
    Ok(())
}

fn check_prerequisites() -> Result<()> {
    require_command("cargo")?;
    require_command("rustc")?;
    require_command("rustup")?;
    require_command("curl")?;
    require_command("sha256sum")?;
    require_command("lsblk")?;
    require_command("elf2uf2-rs")?;

    let installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|error| format!("could not query installed targets: {error}"))?;
    if !installed.status.success() {
        return Err("rustup target list --installed did not succeed".into());
    }
    if !String::from_utf8_lossy(&installed.stdout).contains("thumbv6m-none-eabi") {
        return Err(
            "required target thumbv6m-none-eabi is not installed; run `rustup target add thumbv6m-none-eabi`"
                .into(),
        );
    }
    Ok(())
}

fn require_command(name: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name} >/dev/null 2>&1"))
        .status()
        .map_err(|error| format!("could not probe {name}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("required command missing from PATH: {name}").into())
    }
}

fn build_firmware(workspace_root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            HELLO_CRATE_MANIFEST,
            "--package",
            HELLO_PACKAGE,
            "--target",
            HELLO_TARGET,
            "--release",
        ])
        .current_dir(workspace_root)
        .status()?;
    if !status.success() {
        return Err("cargo build for conduit-pico-w-hello failed".into());
    }
    Ok(())
}

fn convert_uf2(workspace_root: &Path) -> Result<()> {
    let status = Command::new("elf2uf2-rs")
        .arg(&firmware_elf_path(workspace_root))
        .arg(&firmware_uf2_path(workspace_root))
        .current_dir(workspace_root)
        .status()?;
    if !status.success() {
        return Err("elf2uf2-rs conversion failed".into());
    }
    Ok(())
}

fn flash_firmware(
    workspace_root: &Path,
    mount_override: Option<&Path>,
    serial_port: Option<&str>,
) -> Result<PathBuf> {
    let mount = selected_mount(workspace_root, mount_override)?;
    let destination = mount.join(HELLO_UF2_FILE);
    fs::copy(&firmware_uf2_path(workspace_root), &destination)?;
    let status = Command::new("sync").status()?;
    if !status.success() {
        return Err("sync failed after copy".into());
    }

    if let Some(port) = serial_port {
        println!("Connected port hint: {port}");
    }
    println!("Copied {} to {}", firmware_uf2_path(workspace_root).display(), destination.display());
    wait_for_mass_storage_disappear(&mount);
    Ok(mount)
}

fn selected_mount(workspace_root: &Path, override_path: Option<&Path>) -> Result<PathBuf> {
    let explicit = override_path
        .map(std::path::Path::to_path_buf)
        .or_else(|| env::var_os("PICO_W_MOUNT").map(PathBuf::from));

    if let Some(mount) = explicit {
        if !mount.is_dir() {
            return Err(format!("explicit mount is not a directory: {}", mount.display()).into());
        }
        if !is_rpi_rp2_mount(&mount) {
            return Err(format!(
                "explicit mount is not a writable RPI-RP2 volume: {}",
                mount.display()
            )
            .into());
        }
        return Ok(mount);
    }

    let candidates = discover_rpi_rp2_mounts(workspace_root)?;
    if candidates.is_empty() {
        prompt_bootsel(workspace_root)?;
        let candidates = discover_rpi_rp2_mounts(workspace_root)?;
        if candidates.is_empty() {
            return Err("no BOOTSEL RPI-RP2 mount found".into());
        }
        if candidates.len() > 1 {
            return Err(
                "multiple RPI-RP2 volumes are mounted; use --mount <path> to select one".into(),
            );
        }
        return Ok(candidates[0].clone());
    }
    if candidates.len() > 1 {
        return Err(
            "multiple RPI-RP2 volumes detected; use --mount <path> to select one".into(),
        );
    }

    Ok(candidates[0].clone())
}

fn is_rpi_rp2_mount(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let needle = path.to_string_lossy();
    let output = run_output("lsblk", &["-rnpo", "LABEL,PATH,MOUNTPOINT,FSTYPE"]);
    match output {
        Ok(table) => table.lines().any(|line| {
            let mut fields = line.split_whitespace();
            let label = fields.next().unwrap_or("");
            let block_path = fields.next().unwrap_or("");
            let mountpoint = fields.next().unwrap_or("");
            let fstype = fields.next().unwrap_or("");
            label == "RPI-RP2"
                && fstype == "vfat"
                && mountpoint == needle
                && !block_path.is_empty()
        }),
        Err(_) => {
            let has_entry = path.join("SYSTEM").exists() || path.join("INDEX.HTM").exists() || path
                .join("index.htm")
                .exists();
            let fallback = path
                .join("index.js")
                .exists() || path.join("boot.py").exists();
            has_entry || fallback
        }
    }
}

fn discover_rpi_rp2_mounts(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let mut mounts = Vec::new();
    let mut blocks = Vec::new();
    let listing = run_output("lsblk", &["-rnpo", "LABEL,PATH,MOUNTPOINT,FSTYPE"])?;

    for line in listing.lines() {
        let mut fields = line.split_whitespace();
        let label = fields.next();
        if label != Some("RPI-RP2") {
            continue;
        }
        let block = fields.next();
        let mountpoint = fields.next();
        let fstype = fields.next();
        if fstype != Some("vfat") {
            continue;
        }

        if let Some(mountpoint) = mountpoint {
            if mountpoint != "-" && !mountpoint.is_empty() {
                let path = Path::new(mountpoint);
                if path.is_dir() {
                    mounts.push(path.to_path_buf());
                }
            } else if let Some(block) = block {
                blocks.push(block.to_string());
            }
        } else if let Some(block) = block {
            blocks.push(block.to_string());
        }
    }

    if mounts.is_empty() {
        for block in blocks {
            if let Some(path) = mount_with_udisks(&block) {
                if path.is_dir() {
                    mounts.push(path);
                    break;
                }
            }
        }
    }

    let explicit = [
        Path::new("/media").join(format!(
            "{}/RPI-RP2",
            env::var("USER").unwrap_or_else(|_| "root".to_owned())
        )),
        Path::new("/run/media")
            .join(env::var("USER").unwrap_or_else(|_| "root".to_owned()))
            .join("RPI-RP2"),
        Path::new("/media/RPI-RP2").to_path_buf(),
        Path::new("/Volumes/RPI-RP2").to_path_buf(),
        workspace_root.join(".tmp-pico-w-mount"),
    ];

    for path in explicit {
        if path.is_dir() && is_rpi_rp2_mount(&path) && !mounts.contains(&path) {
            mounts.push(path);
        }
    }

    mounts.sort();
    mounts.dedup();
    if !mounts.is_empty() {
        Ok(mounts)
    } else {
        Ok(Vec::new())
    }
}

fn mount_with_udisks(block_path: &str) -> Option<PathBuf> {
    let mount_output = run_output("udisksctl", &["mount", "-b", block_path]).ok()?;
    for line in mount_output.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(pos) = lower.find("at ") {
            let mount = line[pos + 3..].trim().trim_end_matches('.');
            return Some(PathBuf::from(mount));
        }
    }
    None
}

fn prompt_bootsel(workspace_root: &Path) -> Result<()> {
    println!("No mounted RPI-RP2 volume found.");
    println!("Hold BOOTSEL while connecting the Pico W, then press Enter.");
    let mut line = String::new();
    let _ = io::stdout().flush();
    io::stdin().read_line(&mut line)?;
    println!("Waiting up to {HELLO_VERIFY_TIMEOUT_SECONDS}s for a target mount...");

    let mut waited = 0u64;
    while waited < HELLO_VERIFY_TIMEOUT_SECONDS {
        if !discover_rpi_rp2_mounts(workspace_root)?.is_empty() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(HELLO_POLL_INTERVAL_MS));
        waited += 1;
    }

    Err("timed out waiting for BOOTSEL mount".into())
}

fn wait_for_mass_storage_disappear(mount: &Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !mount.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(HELLO_POLL_INTERVAL_MS));
    }
}

fn manifest(workspace_root: &Path) -> Result<IdentityManifest> {
    let manifest_path = identity_manifest_path(workspace_root);
    if !manifest_path.exists() {
        return Err(format!("identity manifest missing: {}", manifest_path.display()).into());
    }
    read_manifest(&manifest_path)
}

fn identity_manifest_path(workspace_root: &Path) -> PathBuf {
    let target_dir = cargo_target_dir(workspace_root);
    target_dir.join(HELLO_TARGET).join(HELLO_PROFILE).join(HELLO_MANIFEST_FILE)
}

fn read_manifest(path: &Path) -> Result<IdentityManifest> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn print_identity(manifest: &IdentityManifest) {
    println!("Target: {}", manifest.target);
    println!("Profile: {}", manifest.profile);
    println!("Conduit revision: {}", manifest.conduit_revision);
    println!("Plan hash: {}", manifest.full_plan_hash);
    println!("Firmware identity: {}", manifest.firmware_identity);
}

fn try_verify_identity(manifest: &IdentityManifest) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(HELLO_VERIFY_TIMEOUT_SECONDS);
    let mut last_err: Option<String> = None;
    let status_timeout = HELLO_STATUS_TIMEOUT_SECONDS.to_string();

    while Instant::now() < deadline {
        match run_output(
            "curl",
            &[
                "-fsS",
                "--max-time",
                &status_timeout,
                HELLO_STATUS_URL,
            ],
        ) {
            Ok(response) => {
                let status: Value = serde_json::from_str(&response)?;
                let firmware_identity = status
                    .get("firmware_identity")
                    .and_then(|value| value.as_str())
                    .ok_or("status.json missing firmware_identity")?;
                let plan_hash = status
                    .get("full_plan_hash")
                    .and_then(|value| value.as_str())
                    .ok_or("status.json missing full_plan_hash")?;

                if firmware_identity != manifest.firmware_identity {
                    return Err(format!(
                        "firmware identity mismatch (built {}, running {firmware_identity})",
                        manifest.firmware_identity,
                    )
                    .into());
                }
                if plan_hash != manifest.full_plan_hash {
                    return Err(format!(
                        "plan hash mismatch (built {}, running {plan_hash})",
                        manifest.full_plan_hash,
                    )
                    .into());
                }
                return Ok(());
            }
            Err(error) => {
                last_err = Some(error.to_string());
                thread::sleep(Duration::from_millis(HELLO_STATUS_RETRY_MS));
            }
        }
    }

    Err(format!(
        "timed out verifying firmware identity: {}",
        last_err.unwrap_or_else(|| "status endpoint unavailable".to_owned())
    )
    .into())
}

fn print_connection_instructions(
    workspace_root: &Path,
    manifest: &IdentityManifest,
    mounted: Option<&Path>,
    serial_port: &Option<String>,
) {
    println!("Artifact path: {}", firmware_uf2_path(workspace_root).display());
    if let Some(path) = mounted {
        println!("Flashed to: {}", path.display());
    }
    if let Some(port) = serial_port {
        println!("Serial port hint: {port}");
    }
    println!("Expected identity:");
    println!("  firmware_identity: {}", manifest.firmware_identity);
    println!("  plan_hash: {}", manifest.full_plan_hash);
    println!("Try: http://192.168.4.1/ and http://192.168.4.1/status.json");
    println!("Try: http://hello.conduit.internal/ and http://gateway.conduit.internal/");
}

fn firmware_elf_path(workspace_root: &Path) -> PathBuf {
    cargo_target_dir(workspace_root)
        .join(HELLO_TARGET)
        .join(HELLO_PROFILE)
        .join(HELLO_PACKAGE)
}

fn firmware_uf2_path(workspace_root: &Path) -> PathBuf {
    cargo_target_dir(workspace_root)
        .join(HELLO_TARGET)
        .join(HELLO_PROFILE)
        .join(HELLO_UF2_FILE)
}

fn cargo_target_dir(workspace_root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("target"))
}

fn cyw43_artifacts_present(workspace_root: &Path) -> bool {
    let directory = workspace_root.join(CYW43_DIR);
    CYW43_FILE_HASHES
        .iter()
        .all(|blob| blob_is_valid(&directory.join(blob.name), blob.sha256).unwrap_or(false))
}

fn blob_is_valid(path: &Path, expected: &str) -> Option<bool> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hasher.finalize();
    Some(
        actual
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            == expected,
    )
}

fn fetch_cyw43_firmware(workspace_root: &Path) -> Result<()> {
    let directory = workspace_root.join(CYW43_DIR);
    fs::create_dir_all(&directory)?;
    let firmware_ref = env::var("CYW43_FIRMWARE_REF").unwrap_or_else(|_| CYW43_FIRMWARE_REF.to_owned());

    for blob in CYW43_FILE_HASHES {
        let target = directory.join(blob.name);
        let command = format!(
            "https://raw.githubusercontent.com/embassy-rs/embassy/{}/cyw43-firmware/{}",
            firmware_ref, blob.name
        );
        let status = Command::new("curl")
            .args(["-fL", "--retry", "3", "--retry-delay", "2", "-o", target.to_string_lossy().as_ref(), &command])
            .status()?;
        if !status.success() {
            return Err(format!("download failed for {}", blob.name).into());
        }
        if !blob_is_valid(&target, blob.sha256).unwrap_or(false) {
            return Err(format!("checksum mismatch for {}", blob.name).into());
        }
    }

    Ok(())
}

fn run_output(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("command failed: {command}: {error}"))?;
    if !output.status.success() {
        return Err(format!("command failed: {command}").into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
