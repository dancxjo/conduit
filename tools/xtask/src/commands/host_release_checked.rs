//! Compile one checked native closure through the existing reviewed Linux adapter.
//! Repository build operation, not an admitted runtime builder or confinement.
use super::*;
use conduit_host_make::{
    build_host_image, verify_image_binding, BuildInputs, HostImage, MakeBuildSelection,
    ReviewedNativeBuild, SporeOutputKind, REVIEWED_NATIVE_BUILD_SCHEMA,
};

pub(super) fn build(
    output: &Path,
    source_identity: &str,
    configuration: &Path,
    library_directories: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    let checked = configuration_load::load_configuration(configuration)?;
    let selection = conduit_workspace_make::package_set()
        .derive_build_selection(checked.profile(), &SporeOutputKind::NativeBundle)
        .map_err(|error| format!("native release Make closure: {error:?}"))?;
    let target = NativeTarget::select(&checked.profile().target.key(), &selection)?;
    let arguments = target.arguments_with_libraries(&selection.features, library_directories)?;
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
    command.args(arguments);
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
    finish_release(
        output,
        source_identity,
        &checked,
        &image,
        selection,
        target.executable,
    )
}

/// Seal the exact compiled IMAGE beside its executable. Configuration still
/// supplies no initialized equipment, directory custody or execution authority.
fn finish_release(
    output: &Path,
    source_identity: &str,
    checked: &conduit_host_make::CheckedHostConfiguration,
    image: &HostImage,
    selection: MakeBuildSelection,
    executable: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let image_bytes = serde_json::to_vec(image)?;
    if image_bytes.len() > 64 * 1024 {
        return Err("checked release IMAGE exceeds its finite manifest bound".into());
    }
    verify_image_binding(image, &image_bytes)
        .map_err(|error| format!("checked release IMAGE: {error:?}"))?;
    fs::write(output.join("host-image.json"), image_bytes)?;
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
        &[
            (executable, "application/vnd.conduit.host+executable"),
            ("host-image.json", "application/vnd.conduit.host-image+json"),
        ],
    )?;
    let path = output.join("checked-native.json");
    let mut manifest: conduit_host_make::HostReleaseManifest =
        serde_json::from_slice(&fs::read(&path)?)?;
    let file = &manifest.files[0];
    manifest.reviewed_native_build = Some(ReviewedNativeBuild {
        schema: REVIEWED_NATIVE_BUILD_SCHEMA.into(),
        configuration_id: checked.configuration_id().into(),
        profile_id: image.manifest.profile_id.clone(),
        build_id: image.manifest.build_id.clone(),
        image_id: image.manifest.image_id.clone(),
        source_identity: source_identity.into(),
        selection,
        executable_path: file.path.clone(),
        executable_sha256: file.sha256.clone(),
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

pub(super) fn native_artifact_name(
    target: &str,
    selection: &MakeBuildSelection,
) -> Result<&'static str, String> {
    NativeTarget::select(target, selection).map(|target| target.executable)
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
    fn arguments_with_libraries(
        &self,
        features: &[String],
        directories: &[PathBuf],
    ) -> Result<Vec<String>, String> {
        let mut args = self.arguments(features);
        if directories.is_empty() {
            return Ok(args);
        }
        if directories.len() > 16 {
            return Err("native SDK exceeds sixteen library directories".into());
        }
        // Only the selected final executable gets these linker inputs. Changing
        // SDK lookup does not recompile dependencies or add a runtime search path.
        args[1] = "rustc".into();
        args.extend(["--bin".into(), "conduit".into(), "--".into()]);
        for directory in directories {
            let path = fs::canonicalize(directory)
                .map_err(|_| "native SDK library directory is unavailable")?;
            if !path.is_dir() {
                return Err("native SDK library input is not a directory".into());
            }
            let path = path.to_str().ok_or("native SDK path is not UTF-8")?;
            // Separate linker arguments preserve spaces and commas as path bytes.
            // rpath-link resolves shared-library dependencies only while linking.
            args.extend([
                "-C".into(),
                format!("link-arg=-L{path}"),
                "-C".into(),
                "link-arg=-Xlinker".into(),
                "-C".into(),
                "link-arg=-rpath-link".into(),
                "-C".into(),
                "link-arg=-Xlinker".into(),
                "-C".into(),
                format!("link-arg={path}"),
            ]);
        }
        Ok(args)
    }

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
            ("std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
                | "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb",
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
