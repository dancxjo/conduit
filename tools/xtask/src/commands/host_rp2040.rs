//! Actual generic Pico Host build. Public code and private native Seed stay separate.
use crate::cli::GlobalOpts;
use conduit_body::{FederationPeer, MembershipCredential, PicoSpawnProvision};
use conduit_host_make::HostImage;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateInput<'a> {
    physical_uid: [u8; 8],
    pinned_owner: FederationPeer,
    #[serde(borrow)]
    provision: PicoSpawnProvision<'a>,
    credential: Option<MembershipCredential>,
}

impl Drop for PrivateInput<'_> {
    fn drop(&mut self) {
        wipe(&mut self.provision.secret);
    }
}

pub(super) fn build(
    profile: &Path,
    output: &Path,
    image: &HostImage,
    description_bytes: &[u8],
    private_input: Option<&Path>,
    private_output: Option<&Path>,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let checked = super::load_configuration(profile)?;
    conduit_host_rp2040::CheckedBoschHt16k33Construction::from_configuration(&checked)?;
    conduit_host_rp2040::check_retained_wifi_construction(&checked)?;
    if opts.dry_run {
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    let state = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&root)
        .output()?;
    if !state.status.success() || !state.stdout.is_empty() {
        return Err("RP2040 firmware requires clean committed Source".into());
    }
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()?;
    if !head.status.success()
        || String::from_utf8(head.stdout)?.trim() != image.manifest.source_identity
    {
        return Err("RP2040 build source identity differs from the exact committed tree".into());
    }
    let manifest = root.join("targets/rp2040/firmware/host/Cargo.toml");
    let profile = profile.canonicalize()?;
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/rp2040-host"));
    let target_dir = if target_dir.is_absolute() {
        target_dir
    } else {
        root.join(target_dir)
    };
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "+stable",
            "build",
            "--locked",
            "--release",
            "--target",
            "thumbv6m-none-eabi",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env("CONDUIT_SOURCE_IDENTITY", &image.manifest.source_identity)
        .env("CONDUIT_HOST_IMAGE_ID", &image.manifest.image_id)
        .env("CONDUIT_HOST_CONFIGURATION", profile)
        .env("CARGO_TARGET_DIR", &target_dir)
        .status()?;
    if !status.success() {
        return Err("RP2040 Host compilation failed".into());
    }
    let post = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&root)
        .output()?;
    let post_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()?;
    if !post.status.success()
        || !post.stdout.is_empty()
        || !post_head.status.success()
        || String::from_utf8(post_head.stdout)?.trim() != image.manifest.source_identity
    {
        return Err("RP2040 Source changed during compilation".into());
    }
    fs::create_dir_all(output)?;
    let elf = target_dir.join("thumbv6m-none-eabi/release/conduit-rp2040-host");
    let uf2 = output.join("host.uf2");
    let status = crate::process::command_for("elf2uf2-rs")
        .arg(&elf)
        .arg(&uf2)
        .status()?;
    if !status.success() {
        return Err("RP2040 UF2 conversion failed".into());
    }
    let bytes = fs::read(&uf2)?;
    validate_application_uf2(&bytes)?;
    // Feed the existing verified artifact/package path used by Crèche. This
    // inventory describes inert bytes; it grants no deployment or membership.
    super::host_release::seal(
        output,
        "checked-artifact.json",
        &image.manifest.target,
        &image.manifest.make_package_id,
        "uf2",
        &image.manifest.builder_adapter,
        image
            .manifest
            .deployment_adapter
            .as_deref()
            .ok_or("RP2040 deployment adapter absent")?,
        &image.manifest.source_identity,
        &[("host.uf2", "application/x-uf2")],
    )?;
    // Use the existing target-owned release extension; no extra payload or
    // competing manifest is needed to select the matching native Seed codec.
    let artifact_path = output.join("checked-artifact.json");
    let mut artifact: conduit_host_make::HostReleaseManifest =
        serde_json::from_slice(&fs::read(&artifact_path)?)?;
    artifact.target_metadata.insert(
        conduit_host_rp2040::BOOTSTRAP_CONTRACT_METADATA.into(),
        serde_json::to_value(conduit_host_rp2040::BootstrapContract::for_image(
            &image.manifest.image_id,
            checked.configuration_id(),
        ))?,
    );
    fs::write(artifact_path, serde_json::to_vec_pretty(&artifact)?)?;
    let elf_digest = format!("{:x}", Sha256::digest(fs::read(&elf)?));
    let uf2_digest = format!("{:x}", Sha256::digest(&bytes));
    // Retain the exact ordinary Make result used to bind this firmware and Seed.
    // The public description contains no private invitation or member key.
    fs::write(output.join("resolved-image.json"), description_bytes)?;
    fs::write(
        output.join("build-manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.rp2040-host/build@1","source_identity":image.manifest.source_identity,
            "configuration_id":checked.configuration_id(),"resolved_description_binding":image.manifest.image_id,
            "target":conduit_host_rp2040::TARGET_ID,"firmware_elf_sha256":elf_digest,
            "application_uf2_sha256":uf2_digest,"body_membership_proven":false,
            "local_sampling_plan_proven":false
        }))?,
    )?;
    if let (Some(input), Some(destination)) = (private_input, private_output) {
        let public = output.canonicalize()?;
        let parent = destination
            .parent()
            .ok_or("private Seed destination requires parent")?
            .canonicalize()?;
        if parent.starts_with(&public) || input.canonicalize()?.starts_with(&public) {
            return Err(
                "private Seed input/output must stay outside the public artifact directory".into(),
            );
        }
        let basis = compose_private_seed(&checked, &image.manifest.image_id, input, destination)?;
        fs::write(
            output.join("usb-admission.json"),
            serde_json::to_vec_pretty(&basis)?,
        )?;
    }
    if !opts.quiet {
        println!("RP2040 application: {}", uf2.display());
    }
    Ok(())
}
pub(super) fn validate_application_uf2(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use conduit_rp2040_flash_layout::{
        classify_image_page, ImageRegion, Uf2Validator, UF2_BLOCK_BYTES,
    };
    let mut validator =
        Uf2Validator::new(bytes.len() as u64).map_err(|e| format!("UF2 extent: {e:?}"))?;
    for block in bytes.as_chunks::<UF2_BLOCK_BYTES>().0 {
        validator
            .push(block)
            .map_err(|e| format!("UF2 validation: {e:?}"))?;
        let address = u32::from_le_bytes(block[12..16].try_into()?);
        let length = u32::from_le_bytes(block[16..20].try_into()?);
        if classify_image_page(address, length) != Ok(ImageRegion::Firmware) {
            return Err("public application reaches private reserved flash".into());
        }
    }
    validator
        .finish()
        .map_err(|e| format!("UF2 incomplete: {e:?}"))?;
    Ok(())
}
fn compose_private_seed(
    checked: &conduit_host_make::CheckedHostConfiguration,
    expected_image_id: &str,
    input: &Path,
    destination: &Path,
) -> Result<conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis, Box<dyn std::error::Error>> {
    let metadata = fs::metadata(input)?;
    if metadata.len() > 4096 {
        return Err("private Seed input exceeds finite bound".into());
    }
    let mut bytes = fs::read(input)?;
    let result = (|| -> Result<conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis, Box<dyn std::error::Error>> {
        let input: PrivateInput<'_> =
            serde_json::from_slice(&bytes).map_err(|_| "invalid private Seed input")?;
        if input.credential.is_some() { return Err("returning members require retained bootstrap material, not a new invitation basis".into()); }
        let member_key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(input.provision.secret)
            .map_err(|_| "private Seed member key refused")?;
        let basis = conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis {
            spore_id: input.provision.spore_id.into(),
            image_id: input.provision.image_id.into(),
            invitation: serde_json::from_value(serde_json::json!({
                "invitation_id": input.provision.invitation_id,
                "body_id": input.provision.body_id,
                "nonce": input.provision.nonce,
                "expires_at_millis": input.provision.expires_at_millis,
            })).map_err(|_| "private Seed invitation claim refused")?,
            expected_host: format!("rp2040/flash/{}", input.physical_uid.iter()
                .map(|byte| format!("{byte:02x}")).collect::<String>()).into(),
            expected_profile: checked.configuration_id().into(),
            member_verifying_key: member_key.verifying_key(),
            owner: input.pinned_owner.clone(),
        };
        let mut sector = conduit_host_rp2040::encode_spawn_membership_seed_sector(
            checked,
            expected_image_id,
            input.physical_uid,
            &input.pinned_owner,
            &input.provision,
        )?;
        let mut private = conduit_host_rp2040::membership_seed_sector_uf2(&sector);
        wipe(&mut sector);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(destination)?;
            file.write_all(&private)?;
            file.sync_all()
        })()
        .map_err(|_| "private Seed publication refused");
        wipe(&mut private);
        result.map(|_| basis).map_err(Into::into)
    })();
    wipe(&mut bytes);
    result
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) }
    }
}
