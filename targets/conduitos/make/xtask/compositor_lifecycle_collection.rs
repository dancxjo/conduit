//! Normal-product artifact custody and I/O, separate from pure wire validation.
use super::{
    ConduitosError, compositor_lifecycle_proof as protocol, profile::Paths, report::sha256_file,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
};
mod embedded;
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("compositor-lifecycle-proof-invalid", detail)
}
fn bytes(path: &Path, maximum: u64) -> Result<Vec<u8>, ConduitosError> {
    if fs::metadata(path)
        .map_err(|e| refusal(e.to_string()))?
        .len()
        > maximum
    {
        return Err(refusal("artifact exceeds observation bound"));
    }
    fs::read(path).map_err(|e| refusal(e.to_string()))
}
fn value(path: &Path) -> Result<Value, ConduitosError> {
    serde_json::from_slice(&bytes(path, 2 * 1024 * 1024)?).map_err(|e| refusal(e.to_string()))
}
fn asset<'a>(manifest: &'a Value, key: &str) -> Result<(&'a str, &'a str), ConduitosError> {
    let file = manifest[key]["file"]
        .as_str()
        .ok_or_else(|| refusal("missing artifact filename"))?;
    let hash = manifest[key]["sha256"]
        .as_str()
        .ok_or_else(|| refusal("missing artifact hash"))?;
    let path = Path::new(file);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(refusal("artifact filename must be a basename"));
    }
    Ok((file, hash))
}
/// Verifies bytes from the already built canonical product; never rebuilds guests.
pub(super) fn collect(
    paths: &Paths,
    image: &Path,
    source: &str,
    digest: &str,
) -> Result<Value, ConduitosError> {
    let folder = image
        .parent()
        .ok_or_else(|| refusal("missing image directory"))?;
    let manifest = value(&folder.join("build-manifest.json"))?;
    if manifest["schema"] != "conduit.host/target-build-manifest@3"
        || manifest["artifact_role"] != "product-host"
        || manifest["target"] != "conduitos/x86_64/pc"
        || manifest["source_identity"] != source
        || manifest["kernel"]["role"] != "freestanding-kernel"
        || manifest["image"]["role"] != "final-bootable-image"
    {
        return Err(refusal(
            "expected exact-source canonical x86_64 product-host build",
        ));
    }
    let (kernel_file, kernel_hash) = asset(&manifest, "kernel")?;
    let (image_file, image_hash) = asset(&manifest, "image")?;
    if image.file_name().and_then(|v| v.to_str()) != Some(image_file)
        || image_hash != digest
        || sha256_file(image)? != digest
    {
        return Err(refusal("actual ISO differs from normal build manifest"));
    }
    let kernel = bytes(&folder.join(kernel_file), 512 * 1024 * 1024)?;
    let root_digest = format!("{:x}", Sha256::digest(&kernel));
    if root_digest != kernel_hash {
        return Err(refusal(
            "actual Root ELF differs from normal build manifest",
        ));
    }
    let resolved_bytes = bytes(&folder.join("resolved-image.json"), 2 * 1024 * 1024)?;
    let resolved: conduit_host_make::HostImage =
        serde_json::from_slice(&resolved_bytes).map_err(|e| refusal(e.to_string()))?;
    conduit_host_make::verify_image_binding(&resolved, &resolved_bytes)
        .map_err(|e| refusal(format!("resolved image: {e:?}")))?;
    let resolved = serde_json::to_value(resolved).map_err(|e| refusal(e.to_string()))?;
    if manifest["resolved_build"] != resolved["manifest"] {
        return Err(refusal(
            "resolved image manifest differs from sealed target manifest",
        ));
    }
    let guest = embedded::extract(&kernel)
        .map_err(|e| refusal(format!("embedded compositor ELF: {e:?}")))?;
    let guest_digest: [u8; 32] = Sha256::digest(guest).into();
    let raw = bytes(&paths.target.join("journey-serial.log"), 64 * 1024 * 1024)?;
    // Terminal collection cannot silently drop an unfinished required record.
    let serial = super::live_serial::complete_records(&raw).map_err(|e| refusal(e.to_string()))?;
    let tail = &raw[serial.len()..];
    if !tail.is_empty()
        && [
            protocol::PREFIX.trim_end().as_bytes(),
            b"CONDUIT_COMPOSITOR_SERVICE_FRAME",
        ]
        .iter()
        .any(|prefix| tail.starts_with(prefix) || prefix.starts_with(tail))
    {
        return Err(refusal(
            "terminal serial contains an incomplete required lifecycle/ACK record",
        ));
    }
    let context = protocol::preparation_context(serial)
        .map_err(|e| refusal(format!("lifecycle protocol: {e:?}")))?;
    let mut boot = None;
    for line in serial
        .lines()
        .filter_map(|l| l.strip_prefix("CONDUIT_BOOT_SIGN "))
    {
        let v: Value = serde_json::from_str(line).map_err(|e| refusal(e.to_string()))?;
        if boot.replace(v).is_some() {
            return Err(refusal("multiple boot scope records"));
        }
    }
    let boot = boot.ok_or_else(|| refusal("missing boot scope record"))?;
    if boot["status"] != "accepted"
        || boot["host_id"] != context.identity.host
        || boot["boot_id"] != context.identity.boot
        || boot["offer_generation"] != context.identity.offer_generation
        || boot["build_id"] != context.identity.make_build
        || boot["profile_id"] != context.identity.make_profile
        || boot["image_binding"] != context.identity.make_image_binding
        || manifest["build_id"] != context.identity.make_build
        || manifest["profile_id"] != context.identity.make_profile
        || manifest["resolved_description_binding"] != context.identity.make_image_binding
        || context.identity.guest_image_sha256 != guest_digest
    {
        return Err(refusal(
            "lifecycle, boot, normal manifest or actual embedded guest binding differs",
        ));
    }
    let installed = serial
        .lines()
        .filter_map(|l| l.strip_prefix(protocol::PREFIX))
        .find_map(|line| {
            let v: Value = serde_json::from_str(line).ok()?;
            (v["snapshot"]["checkpoint"] == "prepared" && v["snapshot"]["kind"] == "installed")
                .then_some(v)
        })
        .ok_or_else(|| refusal("missing installed cost"))?;
    let reserved = installed["snapshot"]["cost"]["reserved_bytes"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| refusal("missing observed service charge"))?;
    let expected = protocol::ExpectedBinding {
        source_commit: source.into(),
        root_elf_sha256: root_digest.clone(),
        iso_sha256: digest.into(),
        identity: context.identity.clone(),
        preparation: context.position.preparation,
        service_reserved_bytes: reserved,
        staging_prepared_bytes: context.staging_prepared_bytes,
    };
    let report = protocol::parse_terminal(serial, &expected)
        .map_err(|e| refusal(format!("lifecycle protocol: {e:?}")))?;
    let snapshots: Vec<Value> = serial
        .lines()
        .filter_map(|l| l.strip_prefix(protocol::PREFIX))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| refusal(e.to_string()))?;
    let selected: Vec<Value> = report.checkpoints.iter().map(|checkpoint| {
        let name = match checkpoint.checkpoint {
            protocol::Checkpoint::Prepared => "prepared", protocol::Checkpoint::InitialHub => "initial-hub",
            protocol::Checkpoint::FirstCreche => "first-creche", protocol::Checkpoint::PlayingHub => "playing-hub",
        };
        let ack = checkpoint.ack.map(|a| json!({"instance":a.instance,"namespace":a.namespace,"frame_sequence":a.frame_sequence,"guest_sequence":a.guest_sequence}));
        json!({"checkpoint":name,"ack":ack,"activation_count":checkpoint.activation_count,"request_count":checkpoint.request_count})
    }).collect();
    let receipt = json!({"schema":"conduit.conduitos/compositor-lifecycle-evidence@1","proof_class":"freestanding-emulator",
        "source_commit":report.source_commit,"root_elf_sha256":report.root_elf_sha256,"image_sha256":report.iso_sha256,
        "guest_image_sha256":format!("{:x}",Sha256::digest(guest)),"guest_elf_bytes":guest.len(),
        "serial_sha256":format!("{:x}",Sha256::digest(&raw)),"instance":report.instance,"selected_checkpoints":selected,
        "lifecycle_records":report.lifecycle_records,"pure_parser_artifact_custody_verified":report.artifact_custody_verified,
        "artifact_custody":{"actual_root_and_iso_hashes_verified":true,"unique_loaded_compositor_entry_elf_verified":true,"normal_manifest_and_boot_join_verified":true},
        "observed_preparation_scope":{"display_base":context.identity.base,"width":context.identity.width,"height":context.identity.height,
            "service_reserved_bytes":reserved,"staging_prepared_bytes":context.staging_prepared_bytes,
            "independently_manifest_declared":false},"snapshots":snapshots,
        "limits":"Source/build and actual Root/ISO bytes are checked; one valid loaded embedded ELF has a unique defined global compositor_entry equal to its entry point. This is structural embedded-byte custody, not authenticated linker invocation, semantic Show proof, calibrated costs, physical display or broad isolation. Display grant and charged service/staging values remain observed preparation scope, not invented manifest facts."});
    fs::write(
        paths.target.join("compositor-lifecycle-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|e| refusal(e.to_string()))?,
    )
    .map_err(|e| refusal(e.to_string()))?;
    Ok(receipt)
}

/// Actual embedded guest custody for a separately built architecture appliance.
pub(super) fn embedded_guest_digest(kernel: &Path) -> Result<[u8; 32], ConduitosError> {
    let root = bytes(kernel, 512 * 1024 * 1024)?;
    let guest =
        embedded::extract(&root).map_err(|e| refusal(format!("embedded compositor ELF: {e:?}")))?;
    Ok(Sha256::digest(guest).into())
}
