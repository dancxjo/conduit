use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, ExecutorKind, Id, ImplementationManifest,
    ManifestArtifactRef, ManifestEntrypoint, NodeContract, PinnedDescriptor, ReplacementSupport,
    SemanticHash,
};
use conduit_runtime::OwnedNodeSchema;

pub struct ProviderFixture {
    pub manifest: &'static ImplementationManifest<'static>,
    pub artifacts: &'static [&'static ArtifactManifest<'static>],
}

pub fn provider(contract: &'static NodeContract<'static>, id: &str) -> ProviderFixture {
    provider_with_contract(
        contract,
        PinnedDescriptor {
            id: contract.id,
            schema_version: 1,
            semantic_hash: OwnedNodeSchema::from_contract(contract).semantic_hash(),
        },
        id,
    )
}

pub fn provider_with_contract(
    contract: &'static NodeContract<'static>,
    semantic_contract: PinnedDescriptor<'static>,
    implementation_id: &str,
) -> ProviderFixture {
    let implementation_id: &'static str = Box::leak(implementation_id.to_owned().into_boxed_str());
    let artifact_id: &'static str =
        Box::leak(format!("{implementation_id}.artifact").into_boxed_str());
    let contract_hash = OwnedNodeSchema::from_contract(contract).semantic_hash();
    let digest = ArtifactDigest::from_bytes(*contract_hash.as_bytes());
    let mut artifact = ArtifactManifest {
        schema_version: 1,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id(artifact_id),
        digest,
        media_type: "application/vnd.conduit.test-provider",
        byte_size: 1,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("conduit/test-provider-builder"),
            source_digest: digest,
            build_recipe_digest: ArtifactDigest::from_bytes([91; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &["Apache-2.0"],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    };
    let mut artifact_scratch =
        vec![SemanticHash::from_bytes([0; 32]); artifact.identity_fact_count()];
    artifact.identity = artifact
        .computed_semantic_hash(&mut artifact_scratch)
        .expect("test artifact identity");
    let artifact = Box::leak(Box::new(artifact));
    let artifact_reference = Box::leak(Box::new([ManifestArtifactRef {
        id: artifact.id,
        digest: artifact.digest,
        role: Id("executable"),
        required: true,
    }]));
    let mut manifest = ImplementationManifest {
        schema_version: 1,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id(implementation_id),
        implementation_version: "1",
        semantic_contract,
        executor: ExecutorKind::NativeInProcess,
        entrypoint: ManifestEntrypoint {
            name: Id("run"),
            adapter: Id("conduit/compatibility-handler-v1"),
            abi: Id("conduit/rust-v1"),
            protocol_version: 1,
        },
        execution_profile: PinnedDescriptor {
            id: Id("conduit/test-execution-profile"),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([92; 32]),
        },
        artifacts: artifact_reference,
        required_interfaces: &[],
        provided_interfaces: &[],
        required_authorities: &[],
        required_effects: &[],
        minimum_plan_version: 1,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        replacement: ReplacementSupport::Cold,
        coexistence_memory_bytes: 0,
        reproducibility: None,
    };
    let mut manifest_scratch =
        vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    manifest.identity = manifest
        .computed_semantic_hash(&mut manifest_scratch)
        .expect("test implementation identity");
    let manifest = Box::leak(Box::new(manifest));
    let artifacts: &'static [&'static ArtifactManifest<'static>] =
        Box::leak(Box::new([&*artifact]));
    ProviderFixture {
        manifest,
        artifacts,
    }
}
