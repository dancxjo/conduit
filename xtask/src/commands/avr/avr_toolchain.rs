use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use super::{require_success, sha256_file};

pub(super) const CLI_VERSION: &str = "1.5.1";
pub(super) const ARDUINO_AVR_VERSION: &str = "1.8.8";
pub(super) const SPARKFUN_AVR_VERSION: &str = "1.1.13";

const CLI_X86_64_SHA256: &str = "28a8e119c498a25607821c36cb2dc49e8463941b261a0d99091baa7bc692dd2b";
const CLI_AARCH64_SHA256: &str = "1e69e077479f300614d4551334e0a33f08ee40b04315d83b8e7e0e94f0d0ee62";
const CLI_X86_64_BINARY_SHA256: &str =
    "cbb47ec4742ee49854031728a0eaeb678a2b36d7a797ef833a1a5b02062149c6";
const CLI_AARCH64_BINARY_SHA256: &str =
    "b878632298958d61fd1eb19e70ac5d2e803d83db8930bc72dc6915eee6e8f433";
const SPARKFUN_INDEX: &str = "https://raw.githubusercontent.com/sparkfun/Arduino_Boards/main/IDE_Board_Manager/package_sparkfun_index.json";

pub(super) fn provision(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let (archive_name, archive_digest, binary_digest) =
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => (
                "arduino-cli_1.5.1_Linux_64bit.tar.gz",
                CLI_X86_64_SHA256,
                CLI_X86_64_BINARY_SHA256,
            ),
            ("linux", "aarch64") => (
                "arduino-cli_1.5.1_Linux_ARM64.tar.gz",
                CLI_AARCH64_SHA256,
                CLI_AARCH64_BINARY_SHA256,
            ),
            _ => return Err("pinned Arduino CLI supports Linux x86_64 or aarch64 only".into()),
        };
    let tools = root.join("target/avr-promicro/tools");
    let cli = tools.join("arduino-cli");
    fs::create_dir_all(&tools)?;
    if !cli.is_file() {
        let archive = tools.join(archive_name);
        let url = format!("https://github.com/arduino/arduino-cli/releases/download/v{CLI_VERSION}/{archive_name}");
        let output = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                &url,
                "--output",
            ])
            .arg(&archive)
            .output()?;
        require_success(&output, "pinned Arduino CLI download")?;
        let found = sha256_file(&archive)?;
        if found != archive_digest {
            return Err(format!(
                "Arduino CLI archive digest mismatch: expected {archive_digest}, found {found}"
            )
            .into());
        }
        let output = Command::new("tar")
            .args(["-xzf"])
            .arg(&archive)
            .args(["-C"])
            .arg(&tools)
            .arg("arduino-cli")
            .output()?;
        require_success(&output, "pinned Arduino CLI extraction")?;
    }
    let found = sha256_file(&cli)?;
    if found != binary_digest {
        return Err(format!(
            "Arduino CLI binary digest mismatch: expected {binary_digest}, found {found}"
        )
        .into());
    }
    write_config(root)?;
    Ok(cli)
}

pub(super) fn verify_cores(cli: &Path, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = config_path(root);
    let update = Command::new(cli)
        .args(["core", "update-index", "--config-file"])
        .arg(&config)
        .output()?;
    require_success(&update, "AVR package index update")?;
    for core in [
        format!("arduino:avr@{ARDUINO_AVR_VERSION}"),
        format!("SparkFun:avr@{SPARKFUN_AVR_VERSION}"),
    ] {
        let output = Command::new(cli)
            .args(["core", "install", &core, "--config-file"])
            .arg(&config)
            .output()?;
        require_success(&output, "pinned AVR core install")?;
    }
    Ok(())
}

pub(super) fn config_path(root: &Path) -> PathBuf {
    root.join("target/avr-promicro/arduino-cli.yaml")
}

pub(super) fn avr_gcc_bin(root: &Path) -> PathBuf {
    root.join("target/avr-promicro/arduino/data/packages/arduino/tools/avr-gcc/7.3.0-atmel3.6.1-arduino7/bin")
}

fn write_config(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let base = root.join("target/avr-promicro/arduino");
    for directory in ["data", "downloads", "user"] {
        fs::create_dir_all(base.join(directory))?;
    }
    let config = format!(
        "board_manager:\n  additional_urls:\n    - {SPARKFUN_INDEX}\ndirectories:\n  data: {}\n  downloads: {}\n  user: {}\n",
        base.join("data").display(),
        base.join("downloads").display(),
        base.join("user").display()
    );
    fs::write(config_path(root), config)?;
    Ok(())
}
