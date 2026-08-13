//! PROFILE-authoritative lowering into a final ConduitOS target artifact.

use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use conduit_host_fabrication::BuildManifest;

use crate::cli::GlobalOpts;

use super::{
    build, image,
    profile::Paths,
    report::{sha256_file, GuestBootSign, GuestPresentationSign},
    ConduitosArch, ConduitosError,
};

#[derive(Debug)]
pub(crate) struct ProfileBuiltImage {
    pub kernel: PathBuf,
    pub image: PathBuf,
    pub kernel_sha256: String,
    pub image_sha256: String,
    pub limine_version: &'static str,
    pub limine_archive_sha256: &'static str,
}

/// Lowers already checked fabrication truth into the existing pinned x86_64
/// compile/link/package machinery. The resolved PROFILE is the authority for
/// entering this path; Cargo features remain an implementation detail until P2
/// makes the full optional-composition mapping exact.
pub(crate) fn build_profile_image(
    manifest: &BuildManifest,
    build_description: &[u8],
    opts: &GlobalOpts,
) -> Result<ProfileBuiltImage, ConduitosError> {
    if manifest.target != "conduitos/x86_64/pc" {
        return Err(ConduitosError::refusal(
            "unsupported-profile-target",
            format!(
                "P0 target lowering owns exactly conduitos/x86_64/pc, found {}",
                manifest.target
            ),
        ));
    }
    if !manifest
        .presenters
        .iter()
        .any(|item| item == "presenter/native-graphical@1")
        || !manifest
            .facilities
            .iter()
            .any(|item| item == "compositor/native@1")
    {
        return Err(ConduitosError::refusal(
            "unsupported-profile-composition",
            "the first bootable target requires the resolved native Presenter and compositor closure",
        ));
    }

    let arch = ConduitosArch::X86_64;
    let build_record = build::execute_profile(manifest, opts)?;
    let image_record = image::assemble_with_description(arch, Some(build_description), opts)?;
    let paths = Paths::new(arch)?;
    Ok(ProfileBuiltImage {
        kernel: paths.kernel,
        image: paths.iso,
        kernel_sha256: build_record.elf_sha256,
        image_sha256: image_record.iso_sha256,
        limine_version: image_record.limine_version,
        limine_archive_sha256: image_record.limine_archive_sha256,
    })
}

pub(crate) fn verify_artifact_digest(
    path: &std::path::Path,
    expected: &str,
) -> Result<(), ConduitosError> {
    let found = sha256_file(path)?;
    if found != expected {
        return Err(ConduitosError::refusal(
            "profile-built-artifact-mismatch",
            format!("{}: expected {expected}, found {found}", path.display()),
        ));
    }
    Ok(())
}

pub(crate) fn boot_profile_image(
    image: &std::path::Path,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-boot-sign",
            "a dry run cannot prove that the final PROFILE-built IMAGE boots",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("profile-built-boot.log");
    let _ = fs::remove_file(&serial_path);
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let image_path = image.to_str().ok_or_else(|| {
        ConduitosError::refusal("profile-built-image-path-invalid", "non-UTF-8 path")
    })?;
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "std",
            "-monitor",
            "none",
            "-serial",
            &serial_target,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            image_path,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let (presentation_json, boot_json) = loop {
        if let Ok(serial) = fs::read_to_string(&serial_path) {
            let reached_front_door = serial
                .lines()
                .filter(|line| *line == "CONDUIT_BOOT_STAGE front-door-presented")
                .count();
            let signs = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_PRESENTATION_SIGN "))
                .collect::<Vec<_>>();
            let boot_signs = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
                .collect::<Vec<_>>();
            if reached_front_door == 1 && signs.len() == 1 && boot_signs.len() == 1 {
                break (signs[0].to_owned(), boot_signs[0].to_owned());
            }
            if reached_front_door > 1 || signs.len() > 1 || boot_signs.len() > 1 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "malformed-profile-built-boot-stage",
                    format!(
                        "expected one front-door stage, presentation Sign, and Boot Sign; found {reached_front_door}, {}, and {}",
                        signs.len(), boot_signs.len()
                    ),
                ));
            }
        }
        if child
            .try_wait()
            .map_err(|error| {
                ConduitosError::refusal("profile-built-boot-failed", error.to_string())
            })?
            .is_some()
        {
            return Err(ConduitosError::refusal(
                "profile-built-boot-failed",
                "QEMU exited before reaching its presented front door",
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConduitosError::refusal(
                "profile-built-boot-timeout",
                "QEMU did not reach its presented front door within 20 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    child
        .kill()
        .and_then(|_| child.wait().map(|_| ()))
        .map_err(|error| {
            ConduitosError::refusal("profile-built-boot-stop-failed", error.to_string())
        })?;
    let presentation: GuestPresentationSign =
        serde_json::from_str(&presentation_json).map_err(|error| {
            ConduitosError::refusal(
                "malformed-profile-built-presentation-sign",
                error.to_string(),
            )
        })?;
    let boot: GuestBootSign = serde_json::from_str(&boot_json).map_err(|error| {
        ConduitosError::refusal("malformed-profile-built-boot-sign", error.to_string())
    })?;
    if presentation.schema != "conduit.conduitos.framebuffer-presentation/v1"
        || presentation.status != "completed"
        || !presentation.completed
    {
        return Err(ConduitosError::refusal(
            "invalid-profile-built-presentation-sign",
            format!("unexpected Presentation Sign: {presentation:?}"),
        ));
    }
    if boot.schema != "conduit.conduitos.boot-sign/v1"
        || boot.status != "accepted"
        || boot.arch != "x86_64"
        || boot.profile_id != expected_profile_id
        || boot.build_id != expected_build_id
        || boot.image_binding != expected_image_binding
        || boot.offer_generation != 1
    {
        return Err(ConduitosError::refusal(
            "profile-built-fabrication-mismatch",
            format!("unexpected artifact-bound Boot Sign: {boot:?}"),
        ));
    }
    if !opts.quiet && !opts.json {
        println!(
            "BOOTED {} to presented front door on Boot {}",
            image.display(),
            presentation.boot_id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use conduit_host_fabrication::{
        build_host_image, BuildInputs, FabricationCatalog, HostBounds, HostProfile,
    };

    use super::*;

    fn resolved(source: &str) -> (BuildManifest, Vec<u8>) {
        let profile: HostProfile = serde_json::from_str(source).unwrap();
        build_host_image(
            profile,
            &FabricationCatalog::canonical(),
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_identity: "test-toolchain".into(),
                toolchain_available: true,
                maxima: HostBounds {
                    static_memory_bytes: u64::MAX,
                    heap_arena_bytes: u64::MAX,
                    queue_items: u32::MAX,
                    buffered_bytes: u64::MAX,
                    active_instances: u32::MAX,
                    operation_slots: u32::MAX,
                    timer_slots: u32::MAX,
                    line_sessions: u32::MAX,
                    evidence_items: u32::MAX,
                },
            },
        )
        .map(|(image, bytes)| (image.manifest, bytes))
        .unwrap()
    }

    #[test]
    fn checked_native_profile_is_the_authority_for_the_first_target_lowering() {
        let (manifest, bytes) = resolved(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        ));
        let built = build_profile_image(
            &manifest,
            &bytes,
            &GlobalOpts {
                dry_run: true,
                ..GlobalOpts::default()
            },
        )
        .unwrap();

        assert_eq!(manifest.target, "conduitos/x86_64/pc");
        assert_eq!(built.image_sha256, "dry-run");
    }

    #[test]
    fn another_target_cannot_fall_into_the_x86_64_product_builder() {
        let (manifest, bytes) = resolved(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        let error = build_profile_image(
            &manifest,
            &bytes,
            &GlobalOpts {
                dry_run: true,
                ..GlobalOpts::default()
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("unsupported-profile-target"));
    }
}
