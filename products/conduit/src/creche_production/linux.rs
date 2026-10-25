//! Concrete Linux package operation; no command, shell or output path from UI.
use super::*;
use conduit_host_hosted::{LinuxSourceBuilder, ReviewedConduitosBootAssets, StdBuilderAdapter};
use conduit_host_make::{
    BuilderBase, BuilderBounds, BuilderCapabilityAdvertisement, BuilderRequest, BuilderSource,
    CheckedHostConfiguration, SporeOutputKind, ToolchainGrant, BUILDER_BASE_IMPLEMENTATION,
    BUILDER_CAPABILITY_SCHEMA, BUILDER_RECEIPT_SCHEMA,
};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

/// Explicit source/toolchain/boot-package/workspace grants from the Host owner.
/// These are not inferred from selecting “Build” or from a source pathname.
pub struct CrecheLinuxBuildGrant {
    pub source: BuilderSource,
    pub expected_source_identity: String,
    pub permitted_local_roots: Vec<PathBuf>,
    pub permitted_network_origins: Vec<String>,
    pub toolchain: ToolchainGrant,
    pub boot_assets: ReviewedConduitosBootAssets,
    pub workspace: PathBuf,
    pub output_directory: PathBuf,
    pub bounds: BuilderBounds,
}

pub(super) fn build(
    context: &conduit_std_host::hosted_creche::CrecheExecutionContext,
    configuration: &CheckedHostConfiguration,
    output: &SporeOutputKind,
    grant: CrecheLinuxBuildGrant,
    remaining: u64,
    cancelled: Arc<AtomicBool>,
) -> Result<(CrecheReviewedArtifact, BuilderReceipt), CrecheProductionRefusal> {
    use CrecheProductionRefusal as E;
    if !grant.bounds.valid()
        || grant.bounds.maximum_output_bytes > remaining
        || grant.expected_source_identity.is_empty()
        || grant.bounds.maximum_evidence_bytes > context.limits().maximum_evidence_bytes
    {
        return Err(E::InvalidBound);
    }
    let packages = conduit_workspace_make::package_set();
    let selection = packages
        .derive_build_selection(configuration.profile(), output)
        .map_err(|e| E::Review(format!("make selection: {e:?}")))?;
    let base = BuilderBase::new(BuilderCapabilityAdvertisement {
        schema: BUILDER_CAPABILITY_SCHEMA.into(),
        implementation_id: BUILDER_BASE_IMPLEMENTATION.into(),
        builder_host_id: context.host().host_id.as_str().into(),
        builder_boot_id: context.host().boot_id.as_str().into(),
        supported_builder_adapters: vec![selection.builder_adapter.clone()],
        maximum_bounds: grant.bounds.clone(),
    })
    .map_err(E::Make)?;
    let backend = LinuxSourceBuilder::new(configuration.clone(), cancelled.clone())
        .with_reviewed_boot_assets(grant.boot_assets);
    let mut adapter = StdBuilderAdapter::new(
        grant.permitted_local_roots,
        grant.permitted_network_origins,
        backend,
    );
    let request = BuilderRequest {
        request_id: format!(
            "creche/image/{}/{}",
            context.play().active_play_id.as_str(),
            configuration.configuration_id()
        ),
        source: grant.source,
        expected_source_identity: Some(grant.expected_source_identity),
        profile: configuration.profile().clone(),
        output: output.clone(),
        toolchain: grant.toolchain,
        workspace: grant.workspace,
        output_directory: grant.output_directory.clone(),
        bounds: grant.bounds,
    };
    let receipt = base
        .make(
            &request,
            &conduit_workspace_make::catalog(),
            &packages,
            &mut adapter,
        )
        .map_err(E::Make)?;
    // Read back the builder's actual resolved description and target receipt.
    // The image binder independently checks both against selected machinery and
    // the actual base ISO digest; successful subprocess exit is insufficient.
    let image: HostImage = serde_json::from_slice(&read_bounded(
        &grant.output_directory.join("resolved-image.json"),
        &cancelled,
    )?)
    .map_err(|_| E::Artifact(PreparedBirthArtifactRefusal::InvalidManifest))?;
    let manifest = read_bounded(
        &grant.output_directory.join("build-manifest.json"),
        &cancelled,
    )?;
    let mut file = File::open(&receipt.artifact.path).map_err(E::Io)?;
    verify_readback(
        &ReadbackExpectation {
            request_id: &request.request_id,
            host_id: context.host().host_id.as_str(),
            boot_id: context.host().boot_id.as_str(),
            source_identity: request
                .expected_source_identity
                .as_deref()
                .ok_or(E::InvalidBound)?,
            toolchain_identity: &request.toolchain.identity,
            package_id: &selection.make_package_id,
            adapter: &selection.builder_adapter,
            maximum_bytes: request.bounds.maximum_output_bytes,
        },
        &receipt,
        &image,
        &manifest,
        &mut file,
        &cancelled,
    )?;
    Ok((
        CrecheReviewedArtifact {
            image,
            target_build_manifest: manifest,
            file,
        },
        receipt,
    ))
}

fn read_bounded(path: &Path, cancelled: &AtomicBool) -> Result<Vec<u8>, CrecheProductionRefusal> {
    let file = File::open(path).map_err(CrecheProductionRefusal::Io)?;
    if !file
        .metadata()
        .map_err(CrecheProductionRefusal::Io)?
        .is_file()
    {
        return Err(CrecheProductionRefusal::Artifact(
            PreparedBirthArtifactRefusal::InvalidManifest,
        ));
    }
    read_small_file(file, cancelled)
}

fn read_small_file(
    mut file: File,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, CrecheProductionRefusal> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(CrecheProductionRefusal::Cancelled);
        }
        let n = file
            .read(&mut buffer)
            .map_err(CrecheProductionRefusal::Io)?;
        if n == 0 {
            break;
        }
        if bytes.len() + n > 1024 * 1024 {
            return Err(CrecheProductionRefusal::InvalidBound);
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    Ok(bytes)
}

struct ReadbackExpectation<'a> {
    request_id: &'a str,
    host_id: &'a str,
    boot_id: &'a str,
    source_identity: &'a str,
    toolchain_identity: &'a str,
    package_id: &'a str,
    adapter: &'a str,
    maximum_bytes: u64,
}

/// Tie independently reopened bytes to the exact operation's returned receipt.
/// The later binder validates image closure, selected configuration and ISO.
fn verify_readback(
    expected: &ReadbackExpectation<'_>,
    receipt: &BuilderReceipt,
    image: &HostImage,
    manifest: &[u8],
    file: &mut File,
    cancelled: &AtomicBool,
) -> Result<(), CrecheProductionRefusal> {
    use sha2::{Digest, Sha256};
    use CrecheProductionRefusal as E;
    let rejected = || E::Artifact(PreparedBirthArtifactRefusal::InvalidManifest);
    if cancelled.load(Ordering::Acquire) {
        return Err(E::Cancelled);
    }
    let resolved: serde_json::Value = serde_json::from_slice(manifest).map_err(|_| rejected())?;
    let m = &image.manifest;
    let from_manifest: conduit_host_make::BuildManifest =
        serde_json::from_value(resolved["resolved_build"].clone()).map_err(|_| rejected())?;
    if receipt.schema != BUILDER_RECEIPT_SCHEMA
        || receipt.builder_implementation_id != BUILDER_BASE_IMPLEMENTATION
        || receipt.request_id != expected.request_id
        || receipt.builder_host_id != expected.host_id
        || receipt.builder_boot_id != expected.boot_id
        || receipt.source_identity != expected.source_identity
        || receipt.toolchain_identity != expected.toolchain_identity
        || receipt.make_package_id != expected.package_id
        || receipt.builder_adapter != expected.adapter
        || receipt.source_identity != m.source_identity
        || receipt.profile_id != m.profile_id
        || receipt.build_id != m.build_id
        || receipt.image_id != m.image_id
        || receipt.toolchain_identity != m.toolchain_identity
        || receipt.make_package_id != m.make_package_id
        || receipt.builder_adapter != m.builder_adapter
        || receipt.target != m.target
        || from_manifest != *m
        || receipt.carrier_realized
        || receipt.boot_observed
        || receipt.destination_host_observed
        || receipt.artifact.bytes == 0
        || receipt.artifact.bytes > expected.maximum_bytes
    {
        return Err(rejected());
    }
    let metadata = file.metadata().map_err(E::Io)?;
    if !metadata.is_file() || metadata.len() != receipt.artifact.bytes {
        return Err(rejected());
    }
    file.seek(SeekFrom::Start(0)).map_err(E::Io)?;
    let mut buffer = [0; 64 * 1024];
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(E::Cancelled);
        }
        let n = file.read(&mut buffer).map_err(E::Io)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or(E::InvalidBound)?;
        if total > receipt.artifact.bytes || total > expected.maximum_bytes {
            return Err(E::InvalidBound);
        }
        hash.update(&buffer[..n]);
    }
    if total != receipt.artifact.bytes
        || format!("sha256:{:x}", hash.finalize()) != receipt.artifact.sha256
        || file.metadata().map_err(E::Io)?.len() != total
    {
        return Err(rejected());
    }
    file.seek(SeekFrom::Start(0)).map_err(E::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests;
