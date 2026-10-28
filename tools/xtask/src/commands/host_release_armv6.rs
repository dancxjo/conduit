//! Opt-in ARM1176 Linux product release; never uses Debian's ARMv7 armhf sysroot.
use super::*;

const TARGET: &str = "arm-unknown-linux-gnueabihf";
const TOOLCHAIN: &str = "1.98.1";
// cross 0.2.5 published amd64 image manifest, resolved from GHCR, not a mutable tag.
// Its crosstool configuration selects armv6, VFP and the hard-float ABI:
// https://github.com/cross-rs/cross/blob/v0.2.5/docker/crosstool-config/arm-unknown-linux-gnueabihf.config
const IMAGE: &str = "ghcr.io/cross-rs/arm-unknown-linux-gnueabihf@sha256:0151546031f7255e58249d57d3307f5f08150373c5597951e1fc61dd753f2ac8";
// Verified with the pinned compiler's -Q --help=target; these are defaults,
// not environment flags forwarded into the container.
const COMPILER_DEFAULTS: &str = "-march=armv6+fp -marm -mfpu=vfp -mfloat-abi=hard";

fn build_command() -> Command {
    let mut command = Command::new("cross");
    command
        .args([
            &format!("+{TOOLCHAIN}"),
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
            "--target",
            TARGET,
        ])
        .env("CROSS_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_IMAGE", IMAGE)
        .env("CROSS_CONTAINER_ENGINE", "docker")
        .env(
            "CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS",
            "-C target-cpu=arm1176jzf-s",
        )
        // Ambient flags must not override the exact CPU profile.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    command
}

pub(super) fn build(
    output: &Path,
    source_identity: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let version = Command::new("cross")
        .arg("--version")
        .output()
        .map_err(|error| format!("install cross 0.2.5 before ARMv6 release: {error}"))?;
    let version_text = String::from_utf8_lossy(&version.stdout);
    if !version.status.success() || !matches!(version_text.split_whitespace().nth(1), Some("0.2.5"))
    {
        return Err("ARMv6 release requires cross 0.2.5".into());
    }
    require_success(&mut build_command(), "compile ARM1176 Linux release")?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(Some(TARGET), "conduit"),
        &output.join("conduit-linux-armv6"),
    )?;
    fs::write(
        output.join("linux-armv6-build.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "conduit.release/armv6-build@1",
            "source_identity": source_identity,
            "rust_toolchain": TOOLCHAIN,
            "target": TARGET,
            "cpu": "arm1176jzf-s",
            "cross_version": "0.2.5",
            "toolchain_image": IMAGE,
            "reviewed_compiler_defaults": COMPILER_DEFAULTS,
            "physical_execution_proved": false,
        }))?,
    )?;
    seal(
        output,
        "raspios-model-b-plus-v1.2.json",
        "std/armv6/raspberry-pi-model-b-plus-v1.2",
        "conduit-host-raspberry-pi@1",
        "native-bundle",
        "conduit-host-raspberry-pi/build-raspios-native@1",
        "conduit-host-raspberry-pi/install-raspios-package@1",
        source_identity,
        &[
            (
                "conduit-linux-armv6",
                "application/vnd.conduit.host+executable",
            ),
            ("linux-armv6-build.json", "application/json"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm1176_build_pins_target_cpu_toolchain_and_sysroot() {
        let command = build_command();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        assert_eq!(
            args,
            [
                "+1.98.1",
                "build",
                "--locked",
                "--release",
                "-p",
                "conduit",
                "--target",
                TARGET
            ]
        );
        let env: std::collections::BTreeMap<_, _> = command.get_envs().collect();
        assert_eq!(
            env[std::ffi::OsStr::new("CROSS_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_IMAGE")],
            Some(std::ffi::OsStr::new(IMAGE))
        );
        assert_eq!(
            env[std::ffi::OsStr::new("CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS")],
            Some(std::ffi::OsStr::new("-C target-cpu=arm1176jzf-s"))
        );
        assert_eq!(env[std::ffi::OsStr::new("RUSTFLAGS")], None);
        assert_eq!(env[std::ffi::OsStr::new("CARGO_ENCODED_RUSTFLAGS")], None);
    }
}
