//! Canonical implementation and immutable-artifact manifest contracts.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    ArtifactDigest, CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id,
    MapField, PinnedDescriptor, SemanticHash,
};

pub const IMPLEMENTATION_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const ARTIFACT_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutorKind {
    NativeInProcess,
    WasmComponent,
    FfiDynamicLibrary,
    Process,
    Firmware,
    RemoteEndpoint,
}

impl ExecutorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeInProcess => "native-in-process",
            Self::WasmComponent => "wasm-component",
            Self::FfiDynamicLibrary => "ffi-dynamic-library",
            Self::Process => "process",
            Self::Firmware => "firmware",
            Self::RemoteEndpoint => "remote-endpoint",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestEntrypoint<'a> {
    pub name: Id<'a>,
    pub adapter: Id<'a>,
    pub abi: Id<'a>,
    pub protocol_version: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestArtifactRef<'a> {
    pub id: Id<'a>,
    pub digest: ArtifactDigest,
    pub role: Id<'a>,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestInterface<'a> {
    pub interface: PinnedDescriptor<'a>,
    pub entrypoint: Id<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementSupport<'a> {
    Cold,
    Quiescent {
        boundary: PinnedDescriptor<'a>,
        maximum_ticks: u64,
    },
    Stateful {
        state_contract: PinnedDescriptor<'a>,
        maximum_export_bytes: u64,
        maximum_import_bytes: u64,
        maximum_ticks: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReproducibilityClaim {
    pub source_digest: ArtifactDigest,
    pub build_recipe_digest: ArtifactDigest,
    pub expected_artifact_digest: ArtifactDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImplementationManifest<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub implementation_version: &'a str,
    pub semantic_contract: PinnedDescriptor<'a>,
    pub executor: ExecutorKind,
    pub entrypoint: ManifestEntrypoint<'a>,
    pub execution_profile: PinnedDescriptor<'a>,
    pub artifacts: &'a [ManifestArtifactRef<'a>],
    pub required_interfaces: &'a [ManifestInterface<'a>],
    pub provided_interfaces: &'a [ManifestInterface<'a>],
    pub required_authorities: &'a [SemanticHash],
    pub required_effects: &'a [SemanticHash],
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    pub minimum_runtime_protocol: u32,
    pub maximum_runtime_protocol: u32,
    pub replacement: ReplacementSupport<'a>,
    pub coexistence_memory_bytes: u64,
    pub reproducibility: Option<ReproducibilityClaim>,
}

impl ImplementationManifest<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        self.artifacts.len()
            + self.required_interfaces.len()
            + self.provided_interfaces.len()
            + self.required_authorities.len()
            + self.required_effects.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, ManifestIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(ManifestIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for artifact in self.artifacts {
            scratch[cursor] = hash_artifact_ref(*artifact)?;
            cursor += 1;
        }
        for interface in self.required_interfaces {
            scratch[cursor] = hash_interface("required", *interface)?;
            cursor += 1;
        }
        for interface in self.provided_interfaces {
            scratch[cursor] = hash_interface("provided", *interface)?;
            cursor += 1;
        }
        for authority in self.required_authorities {
            scratch[cursor] = hash_requirement("authority", *authority)?;
            cursor += 1;
        }
        for effect in self.required_effects {
            scratch[cursor] = hash_requirement("effect", *effect)?;
            cursor += 1;
        }
        let contract = self.semantic_contract;
        let profile = self.execution_profile;
        let entrypoint = entrypoint_fields(&self.entrypoint);
        let replacement = replacement_fields(&self.replacement);
        let reproducibility = self.reproducibility.as_ref().map(reproducibility_fields);
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "implementation_version",
                CanonicalValue::Text(self.implementation_version),
            ),
            semantic(
                "semantic_contract_id",
                CanonicalValue::Identifier(contract.id),
            ),
            semantic(
                "semantic_contract_version",
                CanonicalValue::Integer(i128::from(contract.schema_version)),
            ),
            semantic(
                "semantic_contract_hash",
                CanonicalValue::Bytes(contract.semantic_hash.as_bytes()),
            ),
            semantic(
                "executor",
                CanonicalValue::Identifier(Id(self.executor.as_str())),
            ),
            semantic("entrypoint", CanonicalValue::Map(&entrypoint)),
            semantic(
                "execution_profile_id",
                CanonicalValue::Identifier(profile.id),
            ),
            semantic(
                "execution_profile_version",
                CanonicalValue::Integer(i128::from(profile.schema_version)),
            ),
            semantic(
                "execution_profile_hash",
                CanonicalValue::Bytes(profile.semantic_hash.as_bytes()),
            ),
            semantic(
                "minimum_plan_version",
                CanonicalValue::Integer(i128::from(self.minimum_plan_version)),
            ),
            semantic(
                "maximum_plan_version",
                CanonicalValue::Integer(i128::from(self.maximum_plan_version)),
            ),
            semantic(
                "minimum_runtime_protocol",
                CanonicalValue::Integer(i128::from(self.minimum_runtime_protocol)),
            ),
            semantic(
                "maximum_runtime_protocol",
                CanonicalValue::Integer(i128::from(self.maximum_runtime_protocol)),
            ),
            semantic("replacement", CanonicalValue::Map(&replacement)),
            semantic(
                "coexistence_memory_bytes",
                CanonicalValue::Integer(i128::from(self.coexistence_memory_bytes)),
            ),
            semantic(
                "reproducibility",
                reproducibility
                    .as_ref()
                    .map_or(CanonicalValue::Null, |fields| CanonicalValue::Map(fields)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/implementation-manifest"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(ManifestIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactProvenance<'a> {
    pub builder: Id<'a>,
    pub source_digest: ArtifactDigest,
    pub build_recipe_digest: ArtifactDigest,
    pub reproducible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSignature<'a> {
    pub scheme: Id<'a>,
    pub signer: Id<'a>,
    pub signature_artifact: ArtifactDigest,
    pub provenance_evidence: Option<ArtifactDigest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactLocationKind {
    BundlePath,
    RemoteUri,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactLocation<'a> {
    pub kind: ArtifactLocationKind,
    pub locator: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactManifest<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub digest: ArtifactDigest,
    pub media_type: &'a str,
    pub byte_size: u64,
    pub target: Option<Id<'a>>,
    pub abi: Option<Id<'a>>,
    pub provenance: ArtifactProvenance<'a>,
    pub signatures: &'a [ArtifactSignature<'a>],
    pub license_expressions: &'a [&'a str],
    pub notices: &'a [ManifestArtifactRef<'a>],
    pub sbom: Option<ManifestArtifactRef<'a>>,
    pub source: Option<ManifestArtifactRef<'a>>,
    pub related_artifacts: &'a [ManifestArtifactRef<'a>],
    /// Non-identity retrieval hints. Resolved bytes still require digest
    /// verification before any loader may observe them.
    pub locations: &'a [ArtifactLocation<'a>],
}

impl ArtifactManifest<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        self.signatures.len()
            + self.license_expressions.len()
            + self.notices.len()
            + self.related_artifacts.len()
            + if self.sbom.is_some() { 1 } else { 0 }
            + if self.source.is_some() { 1 } else { 0 }
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, ManifestIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(ManifestIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for signature in self.signatures {
            scratch[cursor] = hash_signature(*signature)?;
            cursor += 1;
        }
        for license in self.license_expressions {
            scratch[cursor] = hash_license(license)?;
            cursor += 1;
        }
        for notice in self.notices {
            scratch[cursor] = hash_tagged_artifact_ref("notice", *notice)?;
            cursor += 1;
        }
        if let Some(sbom) = self.sbom {
            scratch[cursor] = hash_tagged_artifact_ref("sbom", sbom)?;
            cursor += 1;
        }
        if let Some(source) = self.source {
            scratch[cursor] = hash_tagged_artifact_ref("source", source)?;
            cursor += 1;
        }
        for related in self.related_artifacts {
            scratch[cursor] = hash_tagged_artifact_ref("related", *related)?;
            cursor += 1;
        }
        let provenance = provenance_fields(&self.provenance);
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("digest", CanonicalValue::Bytes(self.digest.as_bytes())),
            semantic("media_type", CanonicalValue::Text(self.media_type)),
            semantic(
                "byte_size",
                CanonicalValue::Integer(i128::from(self.byte_size)),
            ),
            semantic(
                "target",
                self.target
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "abi",
                self.abi
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic("provenance", CanonicalValue::Map(&provenance)),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/artifact-manifest"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(ManifestIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestReason {
    UnsupportedSchema,
    InvalidDescriptor,
    IdentityMismatch,
    MissingArtifact,
    UnsupportedVersion,
}

impl ManifestReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "CND-MAN-001",
            Self::InvalidDescriptor => "CND-MAN-002",
            Self::IdentityMismatch => "CND-MAN-003",
            Self::MissingArtifact => "CND-MAN-004",
            Self::UnsupportedVersion => "CND-MAN-005",
        }
    }
}

pub fn validate_implementation_manifest(
    manifest: &ImplementationManifest<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), ManifestReason> {
    if manifest.schema_version != IMPLEMENTATION_MANIFEST_SCHEMA_VERSION {
        return Err(ManifestReason::UnsupportedSchema);
    }
    if !valid_id(manifest.id)
        || manifest.implementation_version.is_empty()
        || !valid_pin(manifest.semantic_contract)
        || !valid_pin(manifest.execution_profile)
        || !valid_entrypoint(manifest.entrypoint)
        || manifest.artifacts.is_empty()
        || manifest
            .artifacts
            .iter()
            .any(|artifact| !valid_id(artifact.id) || !valid_id(artifact.role))
        || manifest
            .required_interfaces
            .iter()
            .chain(manifest.provided_interfaces)
            .any(|interface| !valid_pin(interface.interface) || !valid_id(interface.entrypoint))
        || !valid_replacement(manifest.replacement)
    {
        return Err(ManifestReason::InvalidDescriptor);
    }
    if manifest.minimum_plan_version == 0
        || manifest.minimum_plan_version > manifest.maximum_plan_version
        || manifest.minimum_runtime_protocol == 0
        || manifest.minimum_runtime_protocol > manifest.maximum_runtime_protocol
    {
        return Err(ManifestReason::UnsupportedVersion);
    }
    if !manifest.artifacts.iter().any(|artifact| artifact.required) {
        return Err(ManifestReason::MissingArtifact);
    }
    if manifest.reproducibility.is_some_and(|claim| {
        !manifest
            .artifacts
            .iter()
            .any(|artifact| artifact.required && artifact.digest == claim.expected_artifact_digest)
    }) {
        return Err(ManifestReason::InvalidDescriptor);
    }
    let computed = manifest
        .computed_semantic_hash(scratch)
        .map_err(|_| ManifestReason::InvalidDescriptor)?;
    if computed != manifest.identity {
        return Err(ManifestReason::IdentityMismatch);
    }
    Ok(())
}

pub fn validate_artifact_manifest(
    manifest: &ArtifactManifest<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), ManifestReason> {
    if manifest.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION {
        return Err(ManifestReason::UnsupportedSchema);
    }
    if !valid_id(manifest.id)
        || manifest.media_type.is_empty()
        || manifest.byte_size == 0
        || manifest.target.is_some_and(|target| !valid_id(target))
        || manifest.abi.is_some_and(|abi| !valid_id(abi))
        || !valid_id(manifest.provenance.builder)
        || manifest
            .signatures
            .iter()
            .any(|signature| !valid_id(signature.scheme) || !valid_id(signature.signer))
        || manifest
            .license_expressions
            .iter()
            .any(|license| license.is_empty())
        || manifest
            .notices
            .iter()
            .chain(manifest.related_artifacts)
            .chain(manifest.sbom.iter())
            .chain(manifest.source.iter())
            .any(|reference| !valid_id(reference.id) || !valid_id(reference.role))
        || manifest.locations.iter().any(|location| {
            location.locator.is_empty()
                || (location.kind == ArtifactLocationKind::RemoteUri
                    && !location.locator.contains("://"))
        })
    {
        return Err(ManifestReason::InvalidDescriptor);
    }
    let computed = manifest
        .computed_semantic_hash(scratch)
        .map_err(|_| ManifestReason::InvalidDescriptor)?;
    if computed != manifest.identity {
        return Err(ManifestReason::IdentityMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureVerification<'a> {
    pub signer: Id<'a>,
    pub scheme: Id<'a>,
    pub verified: bool,
    pub verifier: Id<'a>,
    pub evidence_digest: ArtifactDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactTrustPolicy<'a> {
    pub require_signature: bool,
    pub require_provenance_evidence: bool,
    pub require_known_license: bool,
    pub require_sbom: bool,
    pub trusted_signers: &'a [Id<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactVerificationReason {
    DigestMismatch,
    SizeMismatch,
    WrongTarget,
    UnsupportedAbi,
    SignatureRequired,
    SignatureInvalid,
    ProvenanceRequired,
    LicenseRequired,
    SbomRequired,
}

impl ArtifactVerificationReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DigestMismatch => "CND-ART-002",
            Self::SizeMismatch => "CND-ART-003",
            Self::WrongTarget => "CND-ART-004",
            Self::UnsupportedAbi => "CND-ART-005",
            Self::SignatureRequired | Self::SignatureInvalid => "CND-ART-006",
            Self::ProvenanceRequired => "CND-ART-007",
            Self::LicenseRequired => "CND-ART-008",
            Self::SbomRequired => "CND-ART-009",
        }
    }
}

pub fn verify_artifact_candidate(
    manifest: &ArtifactManifest<'_>,
    observed_digest: ArtifactDigest,
    observed_size: u64,
    target: Option<Id<'_>>,
    abi: Option<Id<'_>>,
    policy: ArtifactTrustPolicy<'_>,
    signatures: &[SignatureVerification<'_>],
) -> Result<(), ArtifactVerificationReason> {
    if observed_digest != manifest.digest {
        return Err(ArtifactVerificationReason::DigestMismatch);
    }
    if observed_size != manifest.byte_size {
        return Err(ArtifactVerificationReason::SizeMismatch);
    }
    if manifest.target != target {
        return Err(ArtifactVerificationReason::WrongTarget);
    }
    if manifest.abi != abi {
        return Err(ArtifactVerificationReason::UnsupportedAbi);
    }
    if policy.require_known_license && manifest.license_expressions.is_empty() {
        return Err(ArtifactVerificationReason::LicenseRequired);
    }
    if policy.require_sbom && manifest.sbom.is_none() {
        return Err(ArtifactVerificationReason::SbomRequired);
    }
    let trusted_signature = signatures.iter().any(|verification| {
        verification.verified
            && valid_id(verification.verifier)
            && policy.trusted_signers.contains(&verification.signer)
            && manifest.signatures.iter().any(|declared| {
                declared.signer == verification.signer
                    && declared.scheme == verification.scheme
                    && (!policy.require_provenance_evidence
                        || declared.provenance_evidence == Some(verification.evidence_digest))
            })
    });
    if policy.require_signature && manifest.signatures.is_empty() {
        return Err(ArtifactVerificationReason::SignatureRequired);
    }
    if policy.require_signature && !trusted_signature {
        return Err(ArtifactVerificationReason::SignatureInvalid);
    }
    if policy.require_provenance_evidence && !trusted_signature {
        return Err(ArtifactVerificationReason::ProvenanceRequired);
    }
    Ok(())
}

fn hash_artifact_ref(
    value: ManifestArtifactRef<'_>,
) -> Result<SemanticHash, ManifestIdentityError> {
    hash_tagged_artifact_ref("implementation", value)
}

fn hash_tagged_artifact_ref(
    tag: &str,
    value: ManifestArtifactRef<'_>,
) -> Result<SemanticHash, ManifestIdentityError> {
    let fields = [
        semantic("tag", CanonicalValue::Identifier(Id(tag))),
        semantic("id", CanonicalValue::Identifier(value.id)),
        semantic("digest", CanonicalValue::Bytes(value.digest.as_bytes())),
        semantic("role", CanonicalValue::Identifier(value.role)),
        semantic("required", CanonicalValue::Boolean(value.required)),
    ];
    hash("conduit/manifest-artifact-reference", &fields)
}

fn hash_interface(
    tag: &str,
    value: ManifestInterface<'_>,
) -> Result<SemanticHash, ManifestIdentityError> {
    let fields = [
        semantic("tag", CanonicalValue::Identifier(Id(tag))),
        semantic(
            "interface_id",
            CanonicalValue::Identifier(value.interface.id),
        ),
        semantic(
            "interface_version",
            CanonicalValue::Integer(i128::from(value.interface.schema_version)),
        ),
        semantic(
            "interface_hash",
            CanonicalValue::Bytes(value.interface.semantic_hash.as_bytes()),
        ),
        semantic("entrypoint", CanonicalValue::Identifier(value.entrypoint)),
    ];
    hash("conduit/manifest-interface", &fields)
}

fn hash_requirement(tag: &str, value: SemanticHash) -> Result<SemanticHash, ManifestIdentityError> {
    let fields = [
        semantic("tag", CanonicalValue::Identifier(Id(tag))),
        semantic("requirement_hash", CanonicalValue::Bytes(value.as_bytes())),
    ];
    hash("conduit/manifest-requirement", &fields)
}

fn hash_signature(value: ArtifactSignature<'_>) -> Result<SemanticHash, ManifestIdentityError> {
    if let Some(evidence) = value.provenance_evidence.as_ref() {
        let fields = [
            semantic("scheme", CanonicalValue::Identifier(value.scheme)),
            semantic("signer", CanonicalValue::Identifier(value.signer)),
            semantic(
                "signature_artifact",
                CanonicalValue::Bytes(value.signature_artifact.as_bytes()),
            ),
            semantic(
                "provenance_evidence",
                CanonicalValue::Bytes(evidence.as_bytes()),
            ),
        ];
        hash("conduit/artifact-signature", &fields)
    } else {
        let fields = [
            semantic("scheme", CanonicalValue::Identifier(value.scheme)),
            semantic("signer", CanonicalValue::Identifier(value.signer)),
            semantic(
                "signature_artifact",
                CanonicalValue::Bytes(value.signature_artifact.as_bytes()),
            ),
            semantic("provenance_evidence", CanonicalValue::Null),
        ];
        hash("conduit/artifact-signature", &fields)
    }
}

fn hash_license(value: &str) -> Result<SemanticHash, ManifestIdentityError> {
    let fields = [semantic("expression", CanonicalValue::Text(value))];
    hash("conduit/artifact-license", &fields)
}

fn hash(kind: &str, fields: &[MapField<'_>]) -> Result<SemanticHash, ManifestIdentityError> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
    .map_err(ManifestIdentityError::Canonical)
}

fn entrypoint_fields<'a>(value: &'a ManifestEntrypoint<'a>) -> [MapField<'a>; 4] {
    [
        semantic("name", CanonicalValue::Identifier(value.name)),
        semantic("adapter", CanonicalValue::Identifier(value.adapter)),
        semantic("abi", CanonicalValue::Identifier(value.abi)),
        semantic(
            "protocol_version",
            CanonicalValue::Integer(i128::from(value.protocol_version)),
        ),
    ]
}

fn replacement_fields<'a>(value: &'a ReplacementSupport<'a>) -> [MapField<'a>; 6] {
    let (mode, pin, export, import, ticks) = match value {
        ReplacementSupport::Cold => ("cold", None, 0, 0, 0),
        ReplacementSupport::Quiescent {
            boundary,
            maximum_ticks,
        } => ("quiescent", Some(boundary), 0, 0, *maximum_ticks),
        ReplacementSupport::Stateful {
            state_contract,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        } => (
            "stateful",
            Some(state_contract),
            *maximum_export_bytes,
            *maximum_import_bytes,
            *maximum_ticks,
        ),
    };
    [
        semantic("mode", CanonicalValue::Identifier(Id(mode))),
        semantic(
            "contract_id",
            pin.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Identifier(value.id)
            }),
        ),
        semantic(
            "contract_hash",
            pin.map_or(CanonicalValue::Null, |value| {
                CanonicalValue::Bytes(value.semantic_hash.as_bytes())
            }),
        ),
        semantic(
            "maximum_export_bytes",
            CanonicalValue::Integer(i128::from(export)),
        ),
        semantic(
            "maximum_import_bytes",
            CanonicalValue::Integer(i128::from(import)),
        ),
        semantic("maximum_ticks", CanonicalValue::Integer(i128::from(ticks))),
    ]
}

fn reproducibility_fields(value: &ReproducibilityClaim) -> [MapField<'_>; 3] {
    [
        semantic(
            "source_digest",
            CanonicalValue::Bytes(value.source_digest.as_bytes()),
        ),
        semantic(
            "build_recipe_digest",
            CanonicalValue::Bytes(value.build_recipe_digest.as_bytes()),
        ),
        semantic(
            "expected_artifact_digest",
            CanonicalValue::Bytes(value.expected_artifact_digest.as_bytes()),
        ),
    ]
}

fn provenance_fields<'a>(value: &'a ArtifactProvenance<'a>) -> [MapField<'a>; 4] {
    [
        semantic("builder", CanonicalValue::Identifier(value.builder)),
        semantic(
            "source_digest",
            CanonicalValue::Bytes(value.source_digest.as_bytes()),
        ),
        semantic(
            "build_recipe_digest",
            CanonicalValue::Bytes(value.build_recipe_digest.as_bytes()),
        ),
        semantic("reproducible", CanonicalValue::Boolean(value.reproducible)),
    ]
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id) && value.schema_version > 0
}

fn valid_entrypoint(value: ManifestEntrypoint<'_>) -> bool {
    valid_id(value.name)
        && valid_id(value.adapter)
        && valid_id(value.abi)
        && value.protocol_version > 0
}

fn valid_replacement(value: ReplacementSupport<'_>) -> bool {
    match value {
        ReplacementSupport::Cold => true,
        ReplacementSupport::Quiescent {
            boundary,
            maximum_ticks,
        } => valid_pin(boundary) && maximum_ticks > 0,
        ReplacementSupport::Stateful {
            state_contract,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        } => {
            valid_pin(state_contract)
                && maximum_export_bytes > 0
                && maximum_import_bytes > 0
                && maximum_ticks > 0
        }
    }
}
