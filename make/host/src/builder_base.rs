//! Bounded builder-Host Base contract above the canonical PROFILE -> BUILD -> IMAGE path.

use crate::{
    build_host_image, BuildDiagnostic, BuildInputs, HostImage, HostProfile, MakeCatalog,
    MakePackageSet, SporeOutputKind,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BUILDER_BASE_IMPLEMENTATION: &str = "conduit-host-hosted/make-base@1";
pub const BUILDER_CAPABILITY_SCHEMA: &str = "conduit.make/builder-capability@1";
pub const BUILDER_RECEIPT_SCHEMA: &str = "conduit.make/builder-receipt@1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BuilderSource {
    CurrentCheckout { root: PathBuf },
    LocalCheckout { root: PathBuf },
    RemoteGit { https_url: String, revision: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderBounds {
    pub maximum_source_files: u32,
    pub maximum_source_bytes: u64,
    pub maximum_workspace_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_processes: u16,
    pub maximum_seconds: u64,
    pub maximum_evidence_bytes: u64,
}

impl BuilderBounds {
    pub fn valid(&self) -> bool {
        self.maximum_source_files > 0
            && self.maximum_source_files <= 1_000_000
            && self.maximum_source_bytes > 0
            && self.maximum_workspace_bytes >= self.maximum_source_bytes
            && self.maximum_output_bytes > 0
            && self.maximum_processes > 0
            && self.maximum_processes <= 256
            && self.maximum_seconds > 0
            && self.maximum_seconds <= 86_400
            && self.maximum_evidence_bytes > 0
            && self.maximum_evidence_bytes <= 64 * 1024 * 1024
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainGrant {
    pub identity: String,
    pub allow_acquisition: bool,
    pub permitted_origins: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuilderRequest {
    pub request_id: String,
    pub source: BuilderSource,
    pub expected_source_identity: Option<String>,
    pub profile: HostProfile,
    pub output: SporeOutputKind,
    pub toolchain: ToolchainGrant,
    pub workspace: PathBuf,
    pub output_directory: PathBuf,
    pub bounds: BuilderBounds,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedBuilderSource {
    pub source_identity: String,
    pub revision: String,
    pub content_sha256: String,
    pub root: PathBuf,
    pub files: u32,
    pub bytes: u64,
    pub dirty: bool,
    pub remote_origin: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealizedBuilderArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub evidence_sha256: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderCapabilityAdvertisement {
    pub schema: String,
    pub implementation_id: String,
    pub builder_host_id: String,
    pub builder_boot_id: String,
    pub supported_builder_adapters: Vec<String>,
    pub maximum_bounds: BuilderBounds,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderReceipt {
    pub schema: String,
    pub request_id: String,
    pub builder_implementation_id: String,
    pub builder_host_id: String,
    pub builder_boot_id: String,
    pub source_identity: String,
    pub source_revision: String,
    pub source_content_sha256: String,
    pub profile_id: String,
    pub build_id: String,
    pub image_id: String,
    pub toolchain_identity: String,
    pub make_package_id: String,
    pub builder_adapter: String,
    pub target: String,
    pub artifact: RealizedBuilderArtifact,
    pub carrier_realized: bool,
    pub boot_observed: bool,
    pub destination_host_observed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuilderRefusal {
    InvalidRequest,
    UnauthorizedSource,
    MissingSource,
    StaleRevision,
    DirtySource,
    SourceBoundExhausted,
    UnauthorizedNetwork,
    WorkspaceUnavailable,
    ToolchainUnavailable,
    ToolchainUnauthorized,
    UnsupportedAdapter,
    Profile(Vec<BuildDiagnostic>),
    BuildFailed,
    BoundExhausted,
    MalformedArtifact,
    Interrupted,
}

pub trait BuilderHostAdapter {
    fn seal_source(
        &mut self,
        source: &BuilderSource,
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<SealedBuilderSource, BuilderRefusal>;

    fn prepare_toolchain(
        &mut self,
        required_identity: &str,
        grant: &ToolchainGrant,
        workspace: &Path,
        bounds: &BuilderBounds,
    ) -> Result<(), BuilderRefusal>;

    fn realize(
        &mut self,
        image: &HostImage,
        image_description: &[u8],
        source: &SealedBuilderSource,
        workspace: &Path,
        output: &Path,
        bounds: &BuilderBounds,
    ) -> Result<RealizedBuilderArtifact, BuilderRefusal>;
}

pub struct BuilderBase {
    advertisement: BuilderCapabilityAdvertisement,
}

impl BuilderBase {
    pub fn new(advertisement: BuilderCapabilityAdvertisement) -> Result<Self, BuilderRefusal> {
        if advertisement.schema != BUILDER_CAPABILITY_SCHEMA
            || advertisement.implementation_id != BUILDER_BASE_IMPLEMENTATION
            || advertisement.builder_host_id.is_empty()
            || advertisement.builder_boot_id.is_empty()
            || advertisement.supported_builder_adapters.is_empty()
            || !advertisement.maximum_bounds.valid()
        {
            return Err(BuilderRefusal::InvalidRequest);
        }
        Ok(Self { advertisement })
    }

    pub fn advertisement(&self) -> &BuilderCapabilityAdvertisement {
        &self.advertisement
    }

    pub fn make(
        &self,
        request: &BuilderRequest,
        catalog: &MakeCatalog,
        packages: &MakePackageSet,
        adapter: &mut impl BuilderHostAdapter,
    ) -> Result<BuilderReceipt, BuilderRefusal> {
        validate_request(request, &self.advertisement.maximum_bounds)?;
        let source = adapter.seal_source(&request.source, &request.workspace, &request.bounds)?;
        if source.dirty {
            return Err(BuilderRefusal::DirtySource);
        }
        if let Some(expected) = &request.expected_source_identity {
            if expected != &source.source_identity {
                return Err(BuilderRefusal::StaleRevision);
            }
        }
        let (probe, _) = build_host_image(
            request.profile.clone(),
            catalog,
            packages,
            &request.output,
            &BuildInputs {
                source_identity: source.source_identity.clone(),
                toolchain_available: true,
            },
        )
        .map_err(BuilderRefusal::Profile)?;
        if !self
            .advertisement
            .supported_builder_adapters
            .contains(&probe.manifest.builder_adapter)
        {
            return Err(BuilderRefusal::UnsupportedAdapter);
        }
        if request.toolchain.identity != probe.manifest.toolchain_identity {
            return Err(BuilderRefusal::ToolchainUnauthorized);
        }
        adapter.prepare_toolchain(
            &probe.manifest.toolchain_identity,
            &request.toolchain,
            &request.workspace,
            &request.bounds,
        )?;
        let image_bytes = serde_json::to_vec(&probe).map_err(|_| BuilderRefusal::BuildFailed)?;
        let artifact = adapter.realize(
            &probe,
            &image_bytes,
            &source,
            &request.workspace,
            &request.output_directory,
            &request.bounds,
        )?;
        if artifact.bytes == 0
            || artifact.bytes > request.bounds.maximum_output_bytes
            || artifact.stdout_bytes + artifact.stderr_bytes > request.bounds.maximum_evidence_bytes
            || !artifact.sha256.starts_with("sha256:")
            || !artifact.evidence_sha256.starts_with("sha256:")
        {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        Ok(BuilderReceipt {
            schema: BUILDER_RECEIPT_SCHEMA.into(),
            request_id: request.request_id.clone(),
            builder_implementation_id: self.advertisement.implementation_id.clone(),
            builder_host_id: self.advertisement.builder_host_id.clone(),
            builder_boot_id: self.advertisement.builder_boot_id.clone(),
            source_identity: source.source_identity,
            source_revision: source.revision,
            source_content_sha256: source.content_sha256,
            profile_id: probe.manifest.profile_id.clone(),
            build_id: probe.manifest.build_id.clone(),
            image_id: probe.manifest.image_id.clone(),
            toolchain_identity: probe.manifest.toolchain_identity.clone(),
            make_package_id: probe.manifest.make_package_id.clone(),
            builder_adapter: probe.manifest.builder_adapter.clone(),
            target: probe.manifest.target.clone(),
            artifact,
            carrier_realized: false,
            boot_observed: false,
            destination_host_observed: false,
        })
    }
}

fn validate_request(
    request: &BuilderRequest,
    maximum: &BuilderBounds,
) -> Result<(), BuilderRefusal> {
    if request.request_id.is_empty()
        || !request.bounds.valid()
        || request.workspace == request.output_directory
        || request.toolchain.identity.is_empty()
        || request.bounds.maximum_source_files > maximum.maximum_source_files
        || request.bounds.maximum_source_bytes > maximum.maximum_source_bytes
        || request.bounds.maximum_workspace_bytes > maximum.maximum_workspace_bytes
        || request.bounds.maximum_output_bytes > maximum.maximum_output_bytes
        || request.bounds.maximum_processes > maximum.maximum_processes
        || request.bounds.maximum_seconds > maximum.maximum_seconds
        || request.bounds.maximum_evidence_bytes > maximum.maximum_evidence_bytes
    {
        return Err(BuilderRefusal::InvalidRequest);
    }
    if matches!(&request.source, BuilderSource::RemoteGit { https_url, .. } if !https_url.starts_with("https://"))
    {
        return Err(BuilderRefusal::UnauthorizedNetwork);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_packages::{test_catalog, test_checked_host_profile, test_package_set};
    use sha2::{Digest, Sha256};

    const STD_PROFILE: &str =
        include_str!("../../../targets/std/profiles/std-computer.profile.json");
    const PICO_PROFILE: &str = include_str!("../../../targets/rp2040/profiles/pico-w.host.conduit");

    struct RecordingAdapter {
        source: SealedBuilderSource,
        prepared: Vec<String>,
        artifact: RealizedBuilderArtifact,
    }

    impl BuilderHostAdapter for RecordingAdapter {
        fn seal_source(
            &mut self,
            _: &BuilderSource,
            _: &Path,
            _: &BuilderBounds,
        ) -> Result<SealedBuilderSource, BuilderRefusal> {
            Ok(self.source.clone())
        }

        fn prepare_toolchain(
            &mut self,
            required_identity: &str,
            _: &ToolchainGrant,
            _: &Path,
            _: &BuilderBounds,
        ) -> Result<(), BuilderRefusal> {
            self.prepared.push(required_identity.into());
            Ok(())
        }

        fn realize(
            &mut self,
            _: &HostImage,
            _: &[u8],
            _: &SealedBuilderSource,
            _: &Path,
            _: &Path,
            _: &BuilderBounds,
        ) -> Result<RealizedBuilderArtifact, BuilderRefusal> {
            Ok(self.artifact.clone())
        }
    }

    fn bounds() -> BuilderBounds {
        BuilderBounds {
            maximum_source_files: 10_000,
            maximum_source_bytes: 100_000_000,
            maximum_workspace_bytes: 500_000_000,
            maximum_output_bytes: 10_000_000,
            maximum_processes: 8,
            maximum_seconds: 600,
            maximum_evidence_bytes: 1_000_000,
        }
    }

    fn base(adapter: &str) -> BuilderBase {
        BuilderBase::new(BuilderCapabilityAdvertisement {
            schema: BUILDER_CAPABILITY_SCHEMA.into(),
            implementation_id: BUILDER_BASE_IMPLEMENTATION.into(),
            builder_host_id: "host:builder".into(),
            builder_boot_id: "boot:builder:7".into(),
            supported_builder_adapters: vec![adapter.into()],
            maximum_bounds: bounds(),
        })
        .unwrap()
    }

    fn adapter() -> RecordingAdapter {
        RecordingAdapter {
            source: SealedBuilderSource {
                source_identity: "git:0123456789abcdef0123456789abcdef01234567+sha256:source"
                    .into(),
                revision: "0123456789abcdef0123456789abcdef01234567".into(),
                content_sha256: format!("sha256:{:x}", Sha256::digest(b"source")),
                root: "/workspace/source".into(),
                files: 42,
                bytes: 2048,
                dirty: false,
                remote_origin: None,
            },
            prepared: Vec::new(),
            artifact: RealizedBuilderArtifact {
                path: "/output/host.tar".into(),
                bytes: 1024,
                sha256: format!("sha256:{:x}", Sha256::digest(b"artifact")),
                evidence_sha256: format!("sha256:{:x}", Sha256::digest(b"evidence")),
                stdout_bytes: 40,
                stderr_bytes: 0,
            },
        }
    }

    fn request(profile: HostProfile) -> BuilderRequest {
        BuilderRequest {
            request_id: "make:7".into(),
            source: BuilderSource::CurrentCheckout {
                root: "/src".into(),
            },
            expected_source_identity: None,
            profile,
            output: SporeOutputKind::NativeBundle,
            toolchain: ToolchainGrant {
                identity: "rustc:stable".into(),
                allow_acquisition: true,
                permitted_origins: vec!["https://static.rust-lang.org".into()],
            },
            workspace: "/workspace".into(),
            output_directory: "/output".into(),
            bounds: bounds(),
        }
    }

    #[test]
    fn receipt_correlates_builder_source_profile_build_image_and_artifact() {
        let profile: HostProfile = serde_json::from_str(STD_PROFILE).unwrap();
        let packages = test_package_set();
        let adapter_id = packages
            .target_descriptor("std/x86_64/computer")
            .unwrap()
            .builder_adapter
            .clone();
        let mut adapter = adapter();
        let receipt = base(&adapter_id)
            .make(&request(profile), &test_catalog(), &packages, &mut adapter)
            .unwrap();
        assert_eq!(receipt.builder_host_id, "host:builder");
        assert_eq!(receipt.builder_boot_id, "boot:builder:7");
        assert!(receipt.profile_id.starts_with("sha256:"));
        assert!(receipt.build_id.starts_with("build:sha256:"));
        assert!(receipt.image_id.starts_with("image:sha256:"));
        assert_eq!(receipt.source_content_sha256, adapter.source.content_sha256);
        assert_eq!(adapter.prepared, ["rustc:stable"]);
        assert!(!receipt.carrier_realized);
        assert!(!receipt.boot_observed);
        assert!(!receipt.destination_host_observed);
    }

    #[test]
    fn dirty_stale_unauthorized_toolchain_and_network_are_distinct() {
        let profile: HostProfile = serde_json::from_str(STD_PROFILE).unwrap();
        let packages = test_package_set();
        let adapter_id = packages
            .target_descriptor("std/x86_64/computer")
            .unwrap()
            .builder_adapter
            .clone();
        let builder = base(&adapter_id);

        let mut dirty = adapter();
        dirty.source.dirty = true;
        assert_eq!(
            builder.make(
                &request(profile.clone()),
                &test_catalog(),
                &packages,
                &mut dirty
            ),
            Err(BuilderRefusal::DirtySource)
        );

        let mut stale_request = request(profile.clone());
        stale_request.expected_source_identity = Some("git:other".into());
        assert_eq!(
            builder.make(&stale_request, &test_catalog(), &packages, &mut adapter()),
            Err(BuilderRefusal::StaleRevision)
        );

        let mut wrong_toolchain = request(profile.clone());
        wrong_toolchain.toolchain.identity = "rustc:nightly".into();
        assert_eq!(
            builder.make(&wrong_toolchain, &test_catalog(), &packages, &mut adapter()),
            Err(BuilderRefusal::ToolchainUnauthorized)
        );

        let mut network = request(profile);
        network.source = BuilderSource::RemoteGit {
            https_url: "ssh://forge/repository".into(),
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
        };
        assert_eq!(
            builder.make(&network, &test_catalog(), &packages, &mut adapter()),
            Err(BuilderRefusal::UnauthorizedNetwork)
        );
    }

    #[test]
    fn constrained_selected_base_changes_specialized_build_without_inheritance() {
        let packages = test_package_set();
        let catalog = test_catalog();
        let selected = test_checked_host_profile(PICO_PROFILE);
        let mut omitted = selected.clone();
        omitted.bases.clear();
        omitted.drivers.clear();
        let inputs = BuildInputs {
            source_identity: "git:0123456789abcdef0123456789abcdef01234567+sha256:source".into(),
            toolchain_available: true,
        };
        let (selected_image, _) = build_host_image(
            selected,
            &catalog,
            &packages,
            &SporeOutputKind::Uf2,
            &inputs,
        )
        .unwrap();
        let (omitted_image, _) =
            build_host_image(omitted, &catalog, &packages, &SporeOutputKind::Uf2, &inputs).unwrap();
        assert_ne!(
            selected_image.manifest.build_id,
            omitted_image.manifest.build_id
        );
        assert_ne!(
            selected_image.manifest.image_id,
            omitted_image.manifest.image_id
        );
        assert_eq!(
            selected_image.manifest.builder_adapter,
            "conduit-host-rp2040/build-uf2@1"
        );
        assert!(!selected_image
            .manifest
            .implementations
            .iter()
            .any(|implementation| implementation == BUILDER_BASE_IMPLEMENTATION));
    }
}
