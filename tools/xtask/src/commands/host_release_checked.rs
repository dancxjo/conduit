//! Compile one checked native closure through the existing reviewed Linux adapter.
//! Repository build operation, not an admitted runtime builder or confinement.
use super::*;
use conduit_host_make::{
    build_host_image, BuildInputs, MakeBuildSelection, ReviewedNativeBuild, SporeOutputKind,
    REVIEWED_NATIVE_BUILD_SCHEMA,
};

pub(super) fn build(
    output: &Path,
    source_identity: &str,
    configuration: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let checked = super::super::load_configuration(configuration)?;
    let selection = conduit_workspace_make::package_set()
        .derive_build_selection(checked.profile(), &SporeOutputKind::NativeBundle)
        .map_err(|error| format!("native release Make closure: {error:?}"))?;
    let target = NativeTarget::select(&checked.profile().target.key(), &selection)?;
    verify_source(source_identity)?;
    let (image, _) = build_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &SporeOutputKind::NativeBundle,
        &BuildInputs {
            source_identity: source_identity.into(),
            toolchain_available: true,
        },
    )
    .map_err(|error| format!("checked native release IMAGE: {error:?}"))?;
    let mut command = Command::new("cargo");
    command.args(target.arguments(&selection.features));
    if target.cross {
        command.env(
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER",
            "aarch64-linux-gnu-gcc",
        );
        require_success(
            Command::new("aarch64-linux-gnu-gcc").arg("--version"),
            "AArch64 C linker",
        )?;
        if target.cxx {
            require_success(
                Command::new("aarch64-linux-gnu-g++").arg("--version"),
                "AArch64 C++ compiler",
            )?;
        }
    }
    require_success(&mut command, "compile checked native Host closure")?;
    verify_source(source_identity)?;
    fs::create_dir_all(output)?;
    copy(
        cargo_release_artifact(target.rust_target, "conduit"),
        &output.join(target.executable),
    )?;
    seal(
        output,
        "checked-native.json",
        &image.manifest.target,
        &selection.make_package_id,
        "native-bundle",
        &selection.builder_adapter,
        selection
            .deployment_adapter
            .as_deref()
            .ok_or("native deployment adapter absent")?,
        source_identity,
        &[(target.executable, "application/vnd.conduit.host+executable")],
    )?;
    let path = output.join("checked-native.json");
    let mut manifest: conduit_host_make::HostReleaseManifest =
        serde_json::from_slice(&fs::read(&path)?)?;
    let file = &manifest.files[0];
    manifest.reviewed_native_build = Some(ReviewedNativeBuild {
        schema: REVIEWED_NATIVE_BUILD_SCHEMA.into(),
        configuration_id: checked.configuration_id().into(),
        profile_id: image.manifest.profile_id,
        build_id: image.manifest.build_id,
        image_id: image.manifest.image_id,
        source_identity: source_identity.into(),
        selection,
        executable_path: file.path.clone(),
        executable_sha256: file.sha256.clone(),
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn verify_source(source_identity: &str) -> Result<(), Box<dyn std::error::Error>> {
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if !revision.status.success() || String::from_utf8(revision.stdout)?.trim() != source_identity {
        return Err("checked native release requires the actual full source revision".into());
    }
    let status = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err("checked native release requires a clean source checkout".into());
    }
    Ok(())
}

struct NativeTarget {
    rust_target: Option<&'static str>,
    executable: &'static str,
    cross: bool,
    cxx: bool,
}
impl NativeTarget {
    fn select(target: &str, selection: &MakeBuildSelection) -> Result<Self, String> {
        if selection.features.iter().any(|feature| {
            !matches!(
                feature.as_str(),
                "base-clock"
                    | "base-timer"
                    | "base-serial"
                    | "base-protected-file"
                    | "base-linear-framebuffer"
            )
        }) {
            return Err("native release has unsupported selected build features".into());
        }
        let value = match (target, selection.builder_adapter.as_str(), selection.toolchain_identity.as_str()) {
            ("std/x86_64/computer", "conduit-host-hosted/build-native@1", "rustc:1.98.1") => Self {
                rust_target: Some("x86_64-unknown-linux-gnu"), executable: "conduit-linux-x86_64", cross: false, cxx: false,
            },
            ("std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
             "conduit-host-raspberry-pi/build-raspios-native@1",
             "rustc:1.98.1+aarch64-unknown-linux-gnu+gcc-aarch64-linux-gnu") => Self {
                rust_target: Some("aarch64-unknown-linux-gnu"), executable: "conduit-linux-aarch64", cross: true, cxx: false,
            },
            ("std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb",
             "conduit-host-raspberry-pi/build-raspios-native@1",
             "rustc:1.98.1+aarch64-unknown-linux-gnu+gcc-aarch64-linux-gnu+g++-aarch64-linux-gnu+libc6-dev-arm64-cross") => Self {
                rust_target: Some("aarch64-unknown-linux-gnu"), executable: "conduit-linux-aarch64", cross: true, cxx: true,
            },
            _ => return Err("checked native release target/adapter/toolchain is unsupported".into()),
        };
        Ok(value)
    }
    fn arguments(&self, features: &[String]) -> Vec<String> {
        let mut arguments = [
            "+1.98.1",
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
            "--no-default-features",
        ]
        .map(str::to_owned)
        .to_vec();
        if let Some(target) = self.rust_target {
            arguments.extend(["--target".into(), target.into()]);
        }
        if !features.is_empty() {
            arguments.extend(["--features".into(), features.join(",")]);
        }
        arguments
    }
}

#[cfg(test)]
#[path = "host_release_checked_tests.rs"]
mod tests;
