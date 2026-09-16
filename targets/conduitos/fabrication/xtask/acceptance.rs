//! Exact Crèche-spore admission into the ordinary ConduitOS QEMU journey.

use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use conduit_body::{SpawnInvitationClaim, SpawnInvitationSecret};
use conduit_body_fabrication::{SporeBinding, SporeManifest, SPORE_MANIFEST_SCHEMA};
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli::GlobalOpts;

use super::{journey_proof, profile::Paths, ConduitosArch, ConduitosError};

const PROVISION_SCHEMA: &str = "conduit.spore/native-media-provision@1";
const ACCEPTANCE_SCHEMA: &str = "conduit.conduitos/creche-spore-acceptance@1";
const TARGET: &str = "conduitos/x86_64/pc";
const FABRICATION_PACKAGE: &str = "conduitos-image@1";
const DEPLOYMENT_ADAPTER: &str = "conduit-host-conduitos/boot-x86_64@1";
const MAGIC: &[u8] = b"CONDUIT_SPORE_MEDIA@1\0";
const HEADER_BYTES: usize = 32;
const TRAILER_BYTES: usize = 4096;
const MINIMUM_IMAGE_BYTES: usize = 512;
const MAXIMUM_ARTIFACT_BYTES: usize = 80 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeMediaProvision {
    schema: String,
    image_bytes: usize,
    spore: SporeManifest,
    invitation_provision: InvitationProvision,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationProvision {
    invitation_id: String,
    nonce: Vec<u8>,
    expires_at_millis: u64,
    secret: Vec<u8>,
}

#[derive(Serialize)]
struct AcceptanceProof {
    schema: &'static str,
    source_commit: String,
    spore_path: String,
    spore_id: String,
    body_id: String,
    invitation_id: String,
    invitation_expires_at_millis: u64,
    target: String,
    profile_id: String,
    build_id: String,
    image_id: String,
    image_bytes: usize,
    image_sha256: String,
    artifact_bytes: usize,
    artifact_sha256: String,
    booted_artifact_sha256: String,
    host_id: String,
    boot_id: String,
    fresh_runtime_identity: bool,
    rebuilt_by_harness: bool,
    proof_class: &'static str,
    physical_evidence: bool,
    serial_join_verified: bool,
    membership_claimed_by_guest: bool,
}

#[derive(Debug)]
struct AdmittedSpore {
    provision: NativeMediaProvision,
    image_sha256: String,
    artifact_sha256: String,
    artifact_bytes: usize,
}

pub(super) fn execute(path: &Path, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let admitted = admit(path)?;
    let journey = boot_and_observe(path)?;
    let expected = &admitted.provision.spore;
    if journey.profile_id != expected.profile_id || journey.build_id != expected.build_id {
        return Err(ConduitosError::refusal(
            "creche-spore-guest-binding-mismatch",
            "guest ProfileId or BuildId did not match the admitted Crèche package",
        ));
    }
    if journey.host_id.trim().is_empty() || journey.boot_id.trim().is_empty() {
        return Err(ConduitosError::refusal(
            "creche-spore-runtime-identity-missing",
            "QEMU boot did not originate fresh HostId and BootId truth",
        ));
    }
    validate_serial_join(&admitted, &journey)?;
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let proof_path = paths.target.join("creche-spore-acceptance.json");
    let proof = AcceptanceProof {
        schema: ACCEPTANCE_SCHEMA,
        source_commit: super::report::git_head(&paths.root)?,
        spore_path: path.to_string_lossy().into_owned(),
        spore_id: expected.spore_id.clone(),
        body_id: expected.body_id.clone(),
        invitation_id: admitted
            .provision
            .invitation_provision
            .invitation_id
            .clone(),
        invitation_expires_at_millis: admitted.provision.invitation_provision.expires_at_millis,
        target: expected.target.clone(),
        profile_id: expected.profile_id.clone(),
        build_id: expected.build_id.clone(),
        image_id: expected.image_id.clone(),
        image_bytes: admitted.provision.image_bytes,
        image_sha256: admitted.image_sha256,
        artifact_bytes: admitted.artifact_bytes,
        artifact_sha256: admitted.artifact_sha256.clone(),
        booted_artifact_sha256: admitted.artifact_sha256,
        host_id: journey.host_id,
        boot_id: journey.boot_id,
        fresh_runtime_identity: true,
        rebuilt_by_harness: false,
        proof_class: "freestanding-emulator",
        physical_evidence: false,
        serial_join_verified: true,
        membership_claimed_by_guest: false,
    };
    let bytes = serde_json::to_vec_pretty(&proof).map_err(|error| {
        ConduitosError::refusal("creche-spore-acceptance-invalid", error.to_string())
    })?;
    fs::write(&proof_path, bytes).map_err(|error| {
        ConduitosError::refusal("creche-spore-acceptance-unavailable", error.to_string())
    })?;
    if !opts.quiet && !opts.json {
        println!(
            "ConduitOS Crèche spore acceptance: {}",
            proof_path.display()
        );
    }
    Ok(())
}

fn boot_and_observe(path: &Path) -> Result<journey_proof::JourneyIdentity, ConduitosError> {
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("creche-spore-boot-serial.log");
    let _ = fs::remove_file(&serial_path);
    let serial = format!("file:{}", serial_path.to_string_lossy());
    let image = path.to_str().ok_or_else(|| {
        ConduitosError::refusal("creche-spore-path-invalid", "non-UTF-8 spore path")
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
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            image,
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
    let observed = loop {
        if let Ok(serial) = fs::read_to_string(&serial_path) {
            let joins = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_SPORE_JOIN "))
                .collect::<Vec<_>>();
            let boots = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
                .collect::<Vec<_>>();
            let ready = serial
                .lines()
                .filter(|line| *line == "CONDUIT_BOOT_STAGE front-door-ready")
                .count();
            if joins.len() == 1 && boots.len() == 1 && ready == 1 {
                let join: serde_json::Value = serde_json::from_str(joins[0]).map_err(|error| {
                    ConduitosError::refusal("creche-spore-join-invalid", error.to_string())
                })?;
                let boot: serde_json::Value = serde_json::from_str(boots[0]).map_err(|error| {
                    ConduitosError::refusal("creche-spore-boot-sign-invalid", error.to_string())
                })?;
                break Ok((join, boot));
            }
            if joins.len() > 1 || boots.len() > 1 || ready > 1 {
                break Err(ConduitosError::refusal(
                    "creche-spore-boot-ambiguous",
                    "guest emitted duplicate join, Boot Sign, or front-door readiness",
                ));
            }
        }
        if child
            .try_wait()
            .map_err(|error| {
                ConduitosError::refusal("creche-spore-qemu-wait-failed", error.to_string())
            })?
            .is_some()
        {
            break Err(ConduitosError::refusal(
                "creche-spore-boot-failed",
                "guest exited before emitting its join and ready front door",
            ));
        }
        if Instant::now() >= deadline {
            break Err(ConduitosError::refusal(
                "creche-spore-boot-timeout",
                "guest did not emit its join and ready front door within 20 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let _ = child.kill();
    let _ = child.wait();
    let (join, boot) = observed?;
    let text = |field: &str| {
        boot.get(field)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| ConduitosError::refusal("creche-spore-boot-sign-invalid", field))
    };
    Ok(journey_proof::JourneyIdentity {
        profile_id: text("profile_id")?,
        build_id: text("build_id")?,
        host_id: text("host_id")?,
        boot_id: text("boot_id")?,
        spore_join: Some(join),
    })
}

fn validate_serial_join(
    admitted: &AdmittedSpore,
    journey: &journey_proof::JourneyIdentity,
) -> Result<(), ConduitosError> {
    let join = journey.spore_join.as_ref().ok_or_else(|| {
        ConduitosError::refusal(
            "creche-spore-join-missing",
            "guest did not emit its invitation-bound serial join",
        )
    })?;
    let expected = &admitted.provision.spore;
    if join.get("schema").and_then(serde_json::Value::as_str)
        != Some("conduit.conduitos/serial-spawn-observation@1")
        || join.get("spore_id").and_then(serde_json::Value::as_str)
            != Some(expected.spore_id.as_str())
        || join.get("image_id").and_then(serde_json::Value::as_str)
            != Some(expected.image_id.as_str())
        || join
            .get("invitation_id")
            .and_then(serde_json::Value::as_str)
            != Some(
                admitted
                    .provision
                    .invitation_provision
                    .invitation_id
                    .as_str(),
            )
        || join.get("body_id").and_then(serde_json::Value::as_str)
            != Some(expected.body_id.as_str())
        || join.get("host_id").and_then(serde_json::Value::as_str) != Some(journey.host_id.as_str())
        || join.get("boot_id").and_then(serde_json::Value::as_str) != Some(journey.boot_id.as_str())
        || join
            .get("membership_claimed")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err(ConduitosError::refusal(
            "creche-spore-join-binding-mismatch",
            "guest serial join lost exact spore, invitation, Host, or Boot identity",
        ));
    }
    let advertisement: HostAdvertisement =
        serde_json::from_value(join.get("advertisement").cloned().ok_or_else(|| {
            ConduitosError::refusal("creche-spore-join-invalid", "advertisement absent")
        })?)
        .map_err(|error| ConduitosError::refusal("creche-spore-join-invalid", error.to_string()))?;
    let claim: SpawnInvitationClaim = serde_json::from_value(serde_json::json!({
        "invitation_id": admitted.provision.invitation_provision.invitation_id,
        "body_id": expected.body_id,
        "nonce": admitted.provision.invitation_provision.nonce,
        "expires_at_millis": admitted.provision.invitation_provision.expires_at_millis,
    }))
    .map_err(|error| ConduitosError::refusal("creche-spore-join-invalid", error.to_string()))?;
    let secret_bytes: [u8; 32] = admitted
        .provision
        .invitation_provision
        .secret
        .as_slice()
        .try_into()
        .map_err(|_| ConduitosError::refusal("creche-spore-join-invalid", "secret bound"))?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret_bytes).map_err(|error| {
        ConduitosError::refusal("creche-spore-join-invalid", format!("{error:?}"))
    })?;
    let expected_signature = secret.sign(&claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let signature = join
        .get("signature")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ConduitosError::refusal("creche-spore-join-invalid", "signature absent"))?;
    if signature.len() != expected_signature.len()
        || signature
            .iter()
            .zip(expected_signature)
            .any(|(actual, expected)| {
                actual.as_u64().and_then(|byte| u8::try_from(byte).ok()) != Some(expected)
            })
    {
        return Err(ConduitosError::refusal(
            "creche-spore-join-signature-invalid",
            "guest serial join did not prove the exact invitation transcript",
        ));
    }
    Ok(())
}

fn admit(path: &Path) -> Result<AdmittedSpore, ConduitosError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ConduitosError::refusal("creche-spore-unavailable", error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(ConduitosError::refusal(
            "creche-spore-not-regular-file",
            "spore path must name one regular file, not a symlink or device",
        ));
    }
    let artifact_bytes = usize::try_from(metadata.len()).map_err(|_| {
        ConduitosError::refusal("creche-spore-oversized", "artifact length exceeds usize")
    })?;
    if !(MINIMUM_IMAGE_BYTES..=MAXIMUM_ARTIFACT_BYTES).contains(&artifact_bytes) {
        return Err(ConduitosError::refusal(
            "creche-spore-size-invalid",
            format!("artifact has {artifact_bytes} bytes"),
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| ConduitosError::refusal("creche-spore-unavailable", error.to_string()))?;
    if bytes.len() != artifact_bytes {
        return Err(ConduitosError::refusal(
            "creche-spore-size-changed",
            "artifact size changed while it was being admitted",
        ));
    }
    let provision_offset = locate_provision(&bytes)?;
    let trailer = &bytes[provision_offset..provision_offset + TRAILER_BYTES];
    if trailer.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(ConduitosError::refusal(
            "creche-spore-provision-missing",
            "artifact omitted its native-media provision trailer",
        ));
    }
    let version = u32::from_le_bytes(trailer[24..28].try_into().unwrap());
    let provision_bytes = u32::from_le_bytes(trailer[28..32].try_into().unwrap()) as usize;
    if version != 1 || !(1..=TRAILER_BYTES - HEADER_BYTES).contains(&provision_bytes) {
        return Err(ConduitosError::refusal(
            "creche-spore-provision-header-invalid",
            "native-media provision version or length is unsupported",
        ));
    }
    if trailer[HEADER_BYTES + provision_bytes..]
        .iter()
        .any(|byte| *byte != 0xff)
    {
        return Err(ConduitosError::refusal(
            "creche-spore-provision-padding-invalid",
            "native-media provision padding was modified",
        ));
    }
    let provision: NativeMediaProvision =
        serde_json::from_slice(&trailer[HEADER_BYTES..HEADER_BYTES + provision_bytes]).map_err(
            |error| ConduitosError::refusal("creche-spore-provision-invalid", error.to_string()),
        )?;
    let embedded = provision.image_bytes == artifact_bytes;
    if !embedded && provision.image_bytes != provision_offset {
        return Err(ConduitosError::refusal(
            "creche-spore-provision-binding-invalid",
            "native-media provision did not bind the exact generic IMAGE length",
        ));
    }
    validate_provision(&provision, provision.image_bytes)?;
    let image_sha256 = if embedded {
        let mut generic = bytes.clone();
        generic[provision_offset..provision_offset + TRAILER_BYTES].fill(0xff);
        generic[provision_offset..provision_offset + MAGIC.len()].copy_from_slice(MAGIC);
        generic[provision_offset + 24..provision_offset + 32].fill(0);
        digest(&generic)
    } else {
        digest(&bytes[..provision_offset])
    };
    if provision.spore.image_content_digest != image_sha256 {
        return Err(ConduitosError::refusal(
            "creche-spore-image-tampered",
            format!(
                "manifest expected {}, found {image_sha256}",
                provision.spore.image_content_digest
            ),
        ));
    }
    Ok(AdmittedSpore {
        provision,
        image_sha256,
        artifact_sha256: digest(&bytes),
        artifact_bytes,
    })
}

fn locate_provision(bytes: &[u8]) -> Result<usize, ConduitosError> {
    let offsets = bytes
        .windows(MAGIC.len())
        .enumerate()
        .filter_map(|(offset, value)| {
            if value != MAGIC || offset + TRAILER_BYTES > bytes.len() {
                return None;
            }
            let region = &bytes[offset..offset + TRAILER_BYTES];
            let version = u32::from_le_bytes(region[24..28].try_into().ok()?);
            let length = u32::from_le_bytes(region[28..32].try_into().ok()?) as usize;
            (version == 1 && (1..=TRAILER_BYTES - HEADER_BYTES).contains(&length)).then_some(offset)
        })
        .collect::<Vec<_>>();
    match offsets.as_slice() {
        [offset] if offset + TRAILER_BYTES <= bytes.len() => Ok(*offset),
        [] => Err(ConduitosError::refusal(
            "creche-spore-provision-missing",
            "artifact omitted its native-media provision region",
        )),
        _ => Err(ConduitosError::refusal(
            "creche-spore-provision-ambiguous",
            "artifact contains multiple or truncated native-media provision regions",
        )),
    }
}

fn validate_provision(
    provision: &NativeMediaProvision,
    image_bytes: usize,
) -> Result<(), ConduitosError> {
    if provision.schema != PROVISION_SCHEMA || provision.image_bytes != image_bytes {
        return Err(ConduitosError::refusal(
            "creche-spore-provision-binding-invalid",
            "native-media provision did not bind the exact embedded IMAGE length",
        ));
    }
    let spore = &provision.spore;
    if spore.schema != SPORE_MANIFEST_SCHEMA {
        return Err(ConduitosError::refusal(
            "creche-spore-manifest-schema-unsupported",
            spore.schema.clone(),
        ));
    }
    let invitation_id = match &spore.binding {
        SporeBinding::SelfJoining { invitation_id } => invitation_id,
        SporeBinding::Prejoined { .. } => {
            return Err(ConduitosError::refusal(
                "creche-spore-binding-unsupported",
                "ConduitOS Crèche acceptance requires a self-joining spore",
            ));
        }
    };
    if invitation_id != &provision.invitation_provision.invitation_id
        || provision.invitation_provision.nonce.len() != 32
        || provision.invitation_provision.secret.len() != 32
        || provision.invitation_provision.expires_at_millis == 0
    {
        return Err(ConduitosError::refusal(
            "creche-spore-invitation-invalid",
            "spore and invitation provision lost exact identity or finite secret bounds",
        ));
    }
    if spore.target != TARGET
        || serde_json::to_value(&spore.output).ok().as_ref()
            != Some(&serde_json::Value::String("disk-image".into()))
        || spore.fabrication.fabrication_package_id != FABRICATION_PACKAGE
        || spore.fabrication.deployment_adapter.as_deref() != Some(DEPLOYMENT_ADAPTER)
        || spore.fabrication.output != spore.output
    {
        return Err(ConduitosError::refusal(
            "creche-spore-target-unsupported",
            "artifact is not the exact x86_64 ConduitOS product-host disk-image package",
        ));
    }
    for (name, value) in [
        ("spore_id", spore.spore_id.as_str()),
        ("body_id", spore.body_id.as_str()),
        ("profile_id", spore.profile_id.as_str()),
        ("build_id", spore.build_id.as_str()),
        ("image_id", spore.image_id.as_str()),
    ] {
        if value.is_empty() || value.len() > 192 {
            return Err(ConduitosError::refusal(
                "creche-spore-identity-invalid",
                name,
            ));
        }
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let image = vec![0x5a; 1024];
        let image_digest = digest(&image);
        let provision = serde_json::json!({
            "schema": PROVISION_SCHEMA,
            "image_bytes": image.len(),
            "spore": {
                "schema": SPORE_MANIFEST_SCHEMA,
                "spore_id": "spore:fixture",
                "body_id": "body:fixture",
                "binding": {"mode":"self-joining","invitation_id":"invitation:fixture"},
                "body_description_id": "body-description:fixture",
                "host_entry_name": "host",
                "host_configuration_id": "host-configuration:fixture",
                "profile_id": "profile:fixture",
                "build_id": "build:fixture",
                "image_id": "image:fixture",
                "image_content_digest": image_digest,
                "target": TARGET,
                "output": "disk-image",
                "fabrication": {
                    "fabrication_package_id": FABRICATION_PACKAGE,
                    "fabrication_package_revision": 1,
                    "toolchain_identity": "toolchain:fixture",
                    "builder_adapter": "conduit-host-conduitos/build-x86_64@1",
                    "deployment_adapter": DEPLOYMENT_ADAPTER,
                    "post_build_actions": [],
                    "output": "disk-image",
                    "maxima": {"static_memory_bytes":1,"heap_arena_bytes":0,"queue_items":1,"buffered_bytes":1,"active_instances":1,"operation_slots":1,"timer_slots":1,"line_sessions":1,"evidence_items":1},
                    "features": [],
                    "selected_base_implementations": [],
                    "implementation_packages": []
                },
                "source_identity": "source:fixture"
            },
            "invitation_provision": {
                "invitation_id": "invitation:fixture",
                "nonce": vec![1; 32],
                "expires_at_millis": 1_800_000_000_000_u64,
                "secret": vec![2; 32]
            }
        });
        let encoded = serde_json::to_vec(&provision).unwrap();
        let mut artifact = image;
        artifact.resize(artifact.len() + TRAILER_BYTES, 0xff);
        let offset = artifact.len() - TRAILER_BYTES;
        artifact[offset..offset + MAGIC.len()].copy_from_slice(MAGIC);
        artifact[offset + 24..offset + 28].copy_from_slice(&1_u32.to_le_bytes());
        artifact[offset + 28..offset + 32].copy_from_slice(&(encoded.len() as u32).to_le_bytes());
        artifact[offset + HEADER_BYTES..offset + HEADER_BYTES + encoded.len()]
            .copy_from_slice(&encoded);
        artifact
    }

    #[test]
    fn admits_exact_native_media_and_rejects_image_tampering() {
        let root = std::env::temp_dir().join(format!("conduitos-spore-{}", std::process::id()));
        let _ = fs::remove_file(&root);
        let mut artifact = fixture();
        fs::write(&root, &artifact).unwrap();
        let admitted = admit(&root).unwrap();
        assert_eq!(admitted.provision.image_bytes, 1024);
        assert_ne!(admitted.image_sha256, admitted.artifact_sha256);
        artifact[0] ^= 1;
        fs::write(&root, artifact).unwrap();
        assert_eq!(
            admit(&root).unwrap_err().reason,
            "creche-spore-image-tampered"
        );
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn rejects_wrong_target_and_modified_padding_distinctly() {
        let mut wrong_target = fixture();
        let offset = wrong_target.len() - TRAILER_BYTES;
        let length =
            u32::from_le_bytes(wrong_target[offset + 28..offset + 32].try_into().unwrap()) as usize;
        let document = std::str::from_utf8(
            &wrong_target[offset + HEADER_BYTES..offset + HEADER_BYTES + length],
        )
        .unwrap();
        let replaced = document.replace(TARGET, "conduitos/aarch64/virt");
        wrong_target[offset + 28..offset + 32]
            .copy_from_slice(&(replaced.len() as u32).to_le_bytes());
        wrong_target[offset + HEADER_BYTES..].fill(0xff);
        wrong_target[offset + HEADER_BYTES..offset + HEADER_BYTES + replaced.len()]
            .copy_from_slice(replaced.as_bytes());
        let root =
            std::env::temp_dir().join(format!("conduitos-spore-target-{}", std::process::id()));
        let _ = fs::remove_file(&root);
        fs::write(&root, wrong_target).unwrap();
        assert_eq!(
            admit(&root).unwrap_err().reason,
            "creche-spore-target-unsupported"
        );
        let mut padding = fixture();
        *padding.last_mut().unwrap() = 0;
        fs::write(&root, padding).unwrap();
        assert_eq!(
            admit(&root).unwrap_err().reason,
            "creche-spore-provision-padding-invalid"
        );
        fs::remove_file(root).unwrap();
    }
}
