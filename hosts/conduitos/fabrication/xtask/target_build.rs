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

/// Lowers already checked fabrication truth into the pinned target-specific
/// compile/link/package machinery. The resolved PROFILE and its checked
/// prerequisite closure are the sole authority for optional product inputs.
pub(crate) fn build_profile_image(
    manifest: &BuildManifest,
    build_description: &[u8],
    opts: &GlobalOpts,
) -> Result<ProfileBuiltImage, ConduitosError> {
    let arch = arch_for_target(&manifest.target)?;
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
    target: &str,
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
    let arch = arch_for_target(target)?;
    if arch == ConduitosArch::Aarch64 {
        let first = boot_aarch64_product(
            image,
            expected_profile_id,
            expected_build_id,
            expected_image_binding,
            opts,
        )?;
        let second = boot_aarch64_product(
            image,
            expected_profile_id,
            expected_build_id,
            expected_image_binding,
            opts,
        )?;
        if first["boot_id"] == second["boot_id"] || first["host_id"] == second["host_id"] {
            return Err(ConduitosError::refusal(
                "stale-aarch64-product-identity",
                "independent product boots reused HostId or BootId",
            ));
        }
        let proof = serde_json::json!({
            "schema": "conduit.conduitos/aarch64-product-proof@1",
            "base_commit": super::report::git_head(&Paths::new(arch)?.root)?,
            "image_sha256": sha256_file(image)?,
            "first": first,
            "second": second,
            "fresh_host_id": true,
            "fresh_boot_id": true,
            "stopped_by_harness": true
        });
        fs::write(
            Paths::new(arch)?.target.join("aarch64-product-proof.json"),
            serde_json::to_vec_pretty(&proof).map_err(|error| {
                ConduitosError::refusal("aarch64-product-proof-invalid", error.to_string())
            })?,
        )
        .map_err(|error| {
            ConduitosError::refusal("aarch64-product-proof-unavailable", error.to_string())
        })?;
        return Ok(());
    }
    let paths = Paths::new(arch)?;
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

fn arch_for_target(target: &str) -> Result<ConduitosArch, ConduitosError> {
    let backend = super::target_backend::select(target)?;
    backend.require_machine_boot()?;
    Ok(backend.arch)
}

fn boot_aarch64_product(
    image: &std::path::Path,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
    opts: &GlobalOpts,
) -> Result<serde_json::Value, ConduitosError> {
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let firmware = std::env::var_os("CONDUITOS_AARCH64_FIRMWARE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/usr/share/qemu-efi-aarch64/QEMU_EFI.fd"));
    if !firmware.is_file() {
        return Err(ConduitosError::refusal(
            "unavailable-aarch64-firmware",
            firmware.display().to_string(),
        ));
    }
    let image_path = image.to_str().ok_or_else(|| {
        ConduitosError::refusal("profile-built-image-path-invalid", "non-UTF-8 path")
    })?;
    let serial_path = paths.target.join("aarch64-product-boot.log");
    let _ = fs::remove_file(&serial_path);
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let mut child = Command::new("qemu-system-aarch64")
        .args([
            "-M",
            "virt",
            "-cpu",
            "cortex-a72",
            "-m",
            "256M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            &serial_target,
            "-net",
            "none",
            "-no-reboot",
            "-bios",
        ])
        .arg(&firmware)
        .args(["-cdrom", image_path, "-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-aarch64-qemu", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let transcript = fs::read_to_string(&serial_path).unwrap_or_default();
        if let Some(json) = complete_aarch64_product_sign(&transcript) {
            let value: serde_json::Value = serde_json::from_str(json).map_err(|error| {
                ConduitosError::refusal("malformed-aarch64-product-sign", error.to_string())
            })?;
            if let Err(error) = validate_aarch64_product_sign(
                &value,
                expected_profile_id,
                expected_build_id,
                expected_image_binding,
            ) {
                let _ = child.kill();
                return Err(error);
            }
            thread::sleep(Duration::from_millis(250));
            if child
                .try_wait()
                .map_err(|error| {
                    ConduitosError::refusal("aarch64-product-wait-failed", error.to_string())
                })?
                .is_some()
            {
                return Err(ConduitosError::refusal(
                    "aarch64-product-not-long-lived",
                    "product Host exited after ready Sign",
                ));
            }
            child
                .kill()
                .and_then(|_| child.wait().map(|_| ()))
                .map_err(|error| {
                    ConduitosError::refusal("aarch64-product-stop-failed", error.to_string())
                })?;
            if !opts.quiet && !opts.json {
                println!(
                    "BOOTED {} to AArch64 linear product front door",
                    image.display()
                );
            }
            return Ok(value);
        }
        if child
            .try_wait()
            .map_err(|error| {
                ConduitosError::refusal("aarch64-product-wait-failed", error.to_string())
            })?
            .is_some()
        {
            return Err(ConduitosError::refusal(
                "aarch64-product-exited-early",
                transcript,
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(ConduitosError::refusal(
                "aarch64-product-timeout",
                transcript,
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn complete_aarch64_product_sign(transcript: &str) -> Option<&str> {
    const PREFIX: &str = "CONDUIT_AARCH64_PRODUCT ";
    transcript.lines().find_map(|line| {
        line.find(PREFIX)
            .map(|offset| &line[offset + PREFIX.len()..])
            .filter(|json| json.ends_with('}'))
    })
}

fn validate_aarch64_product_sign(
    value: &serde_json::Value,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
) -> Result<(), ConduitosError> {
    if value["schema"] != "conduit.conduitos/aarch64-product@1"
        || value["status"] != "ready"
        || value["profile_id"] != expected_profile_id
        || value["build_id"] != expected_build_id
        || value["image_id"] != expected_image_binding
        || value["host_id"].as_str().is_none_or(str::is_empty)
        || value["boot_id"].as_str().is_none_or(str::is_empty)
        || value["body_id"] != serde_json::Value::Null
        || value["interactive_local_control"] != false
        || value["long_lived"] != true
        || value["semantic_result"] != "HELLO, CONDUITOS"
        || value["presenter_implementation_id"] != "presenter/linear-serial@1"
    {
        return Err(ConduitosError::refusal(
            "profile-built-fabrication-mismatch",
            value.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use conduit_host_fabrication::{build_default_host_image, BuildInputs, HostProfile};

    use super::*;

    fn resolved(source: &str) -> (BuildManifest, Vec<u8>) {
        let profile: HostProfile = serde_json::from_str(source).unwrap();
        build_default_host_image(
            profile,
            &conduit_workspace_fabrication::catalog(),
            &conduit_workspace_fabrication::package_set(),
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_available: true,
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
    fn checked_headless_profile_enters_the_same_authoritative_target_lowering() {
        let (manifest, bytes) = resolved(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
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
    fn checked_aarch64_profile_routes_to_the_distinct_product_artifact() {
        let (manifest, bytes) = resolved(include_str!(
            "../../../../profiles/hosts/conduitos-aarch64-headless.profile.json"
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
        assert_eq!(manifest.target, "conduitos/aarch64/virt");
        assert_eq!(built.image_sha256, "dry-run");
        assert!(arch_for_target("conduitos/aarch64/a3-proof").is_err());
    }

    #[test]
    fn aarch64_product_sign_rejects_stale_bindings_and_false_capabilities() {
        let exact = serde_json::json!({
            "schema": "conduit.conduitos/aarch64-product@1",
            "status": "ready",
            "profile_id": "profile",
            "build_id": "build",
            "image_id": "image",
            "host_id": "host",
            "boot_id": "boot",
            "body_id": null,
            "interactive_local_control": false,
            "long_lived": true,
            "semantic_result": "HELLO, CONDUITOS",
            "presenter_implementation_id": "presenter/linear-serial@1"
        });
        assert!(validate_aarch64_product_sign(&exact, "profile", "build", "image").is_ok());
        for (field, stale) in [
            ("profile_id", "stale-profile"),
            ("build_id", "stale-build"),
            ("image_id", "stale-image"),
            (
                "presenter_implementation_id",
                "presenter/native-graphical@1",
            ),
        ] {
            let mut malformed = exact.clone();
            malformed[field] = stale.into();
            assert!(
                validate_aarch64_product_sign(&malformed, "profile", "build", "image").is_err()
            );
        }
    }

    #[test]
    fn incomplete_aarch64_product_sign_is_not_promoted_or_rejected_early() {
        assert_eq!(
            complete_aarch64_product_sign(
                "firmware\0CONDUIT_AARCH64_PRODUCT {\"schema\":\"partial"
            ),
            None
        );
        assert_eq!(
            complete_aarch64_product_sign(
                "firmware\0CONDUIT_AARCH64_PRODUCT {\"schema\":\"complete\"}\n"
            ),
            Some("{\"schema\":\"complete\"}")
        );
    }
}
