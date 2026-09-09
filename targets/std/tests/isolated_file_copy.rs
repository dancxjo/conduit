#![cfg(all(target_os = "linux", feature = "isolated-file-base"))]

use conduit_core::{
    ActivePlayIdentity, AuthorityContractId, AuthorityGrant, AuthorityGrantId,
    BaseCapabilityAuthority, BaseCapabilityScope, BaseOperationClaim, BootId, CapabilityEnvelopeId,
    CapabilityId, CapabilityIssueRequest, GearId, HostId, HostOperationContractId, KindId,
    OfferGeneration, PlanFragment, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceGenerationId, ResourceHandleId,
    ResourcePoolId,
};
use conduit_std_host::{
    prepare_isolated_copy_task, CopyRequestId, CopyResult, CopyStopToken, IsolatedFileBaseConfig,
    ProtectedFileAvailability, ProtectedFileRegistry, StdHostComposition, StdHostConfig,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-isolated-copy-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir(&path).unwrap();
        path
    }
}

fn run_isolated(
    source: &Path,
    destination: &Path,
    policy: ProtectedResourceCommitPolicy,
    maximum_bytes: u64,
    stopped: bool,
    executable: Option<&Path>,
) -> conduit_std_host::CopyRunReceipt {
    let provider = IsolatedFileBaseConfig {
        executable: executable.map_or_else(
            || PathBuf::from(env!("CARGO_BIN_EXE_conduit-isolated-copy-base")),
            Path::to_path_buf,
        ),
        base_instance_id: conduit_core::BaseInstanceId::from("base/files/provider/semantics"),
        provider_generation: 7,
    };
    let mut host = conduit_std_host::isolated_copy_base::IsolatedFileHost::new(
        StdHostConfig {
            host_id: HostId::from("isolated-copy-host"),
            boot_id: BootId::from("isolated-copy-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        provider,
    )
    .unwrap();
    let mut registry = ProtectedFileRegistry::default();
    let source_grant = register(
        &mut registry,
        source,
        conduit_semantic_catalog::COPY_SOURCE_ROLE,
        ProtectedResourceAccess::ReadExisting,
        ProtectedResourceCommitPolicy::NotApplicable,
        maximum_bytes,
    );
    let destination_grant = register(
        &mut registry,
        destination,
        conduit_semantic_catalog::COPY_DESTINATION_ROLE,
        if policy == ProtectedResourceCommitPolicy::CreateOnly {
            ProtectedResourceAccess::Create
        } else {
            ProtectedResourceAccess::Replace
        },
        policy,
        maximum_bytes,
    );
    let prepared = prepare_isolated_copy_task(
        host.host(),
        &[source_grant, destination_grant],
        &authority(),
    )
    .unwrap();
    let play = host
        .host_mut()
        .issue_kernel_play(&prepared.fragment)
        .unwrap();
    let stop = CopyStopToken::default();
    if stopped {
        stop.request_stop();
    }
    host.run_copy_fragment(
        play,
        CopyRequestId::new("isolated/semantics").unwrap(),
        prepared.fragment,
        &mut registry,
        &stop,
    )
    .unwrap()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn register(
    registry: &mut ProtectedFileRegistry,
    path: &Path,
    role: &str,
    access: ProtectedResourceAccess,
    policy: ProtectedResourceCommitPolicy,
    maximum_bytes: u64,
) -> conduit_core::ProtectedResourceGrant {
    registry
        .register(
            ResourceHandleId::from(format!("handle/{role}")),
            path,
            GearId::from("copy-task/task"),
            ResourceBindingRoleId::from(role),
            HostId::from("isolated-copy-host"),
            BootId::from("isolated-copy-boot"),
            CapabilityId::from(conduit_std_offers::COPY_FILE_CAPABILITY),
            access,
            maximum_bytes,
            policy,
            ProtectedFileAvailability::Available,
        )
        .unwrap()
}

fn authority() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/isolated-copy"),
        contract_id: AuthorityContractId::from(
            conduit_std_offers::ISOLATED_COPY_FILE_AUTHORITY_CONTRACT,
        ),
        host_operation_contract_id: HostOperationContractId::from(
            conduit_std_offers::COPY_FILE_HOST_OPERATION_CONTRACT,
        ),
        subject_kind: KindId::from(conduit_semantic_catalog::COPY_FILE_KIND),
        host_id: HostId::from("isolated-copy-host"),
        boot_id: BootId::from("isolated-copy-boot"),
        capability_id: CapabilityId::from(conduit_std_offers::COPY_FILE_CAPABILITY),
    }
}

#[allow(clippy::too_many_arguments)]
fn proof_material(
    fragment: &PlanFragment,
    play: &ActivePlayIdentity,
    provider: &IsolatedFileBaseConfig,
    source_grant: &ProtectedResourceGrant,
    destination_grant: &ProtectedResourceGrant,
    source: &Path,
    destination: &Path,
    maximum_bytes: u64,
) -> (
    conduit_std_host::isolated_copy_base::CopyBootstrap,
    BaseOperationClaim,
) {
    let placement = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::COPY_FILE_KIND)
        .unwrap();
    let binding = placement.authority.first().unwrap();
    let resource = format!(
        "{}+{}",
        source_grant.handle_id.as_str(),
        destination_grant.handle_id.as_str()
    );
    let envelope = CapabilityEnvelopeId::from(format!(
        "protected-file-pair/{}/{}/{}",
        source_grant.handle_id.as_str(),
        destination_grant.handle_id.as_str(),
        maximum_bytes
    ));
    let operation =
        HostOperationContractId::from(conduit_std_offers::COPY_FILE_HOST_OPERATION_CONTRACT);
    let resource_pool = ResourcePoolId::from(resource.clone());
    let resource_generation = ResourceGenerationId(resource);
    let operations = maximum_bytes
        .div_ceil(conduit_semantic_catalog::COPY_CHUNK_BYTES as u64)
        .saturating_add(1) as u32;
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: provider.base_instance_id.clone(),
        base_provider_generation: provider.provider_generation,
        plan_id: fragment.plan_id.clone(),
        active_play_id: play.active_play_id.clone(),
        authority_grant_id: binding.grant_id.clone(),
        authority_contract_id: binding.contract_id.clone(),
        capability_id: placement.capability_id.clone(),
        implementation_id: placement.implementation_id.clone(),
        operation_contract_id: operation.clone(),
        subject_kind: placement.kind_id.clone(),
        resource_pool_id: resource_pool.clone(),
        resource_generation_id: resource_generation.clone(),
        envelope_id: envelope.clone(),
        maximum_parameter_bytes: 1,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: operations,
    };
    let grant = AuthorityGrant {
        grant_id: binding.grant_id.clone(),
        contract_id: binding.contract_id.clone(),
        host_operation_contract_id: binding.host_operation_contract_id.clone(),
        subject_kind: binding.subject_kind.clone(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        capability_id: binding.capability_id.clone(),
    };
    let issue = CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant,
            base_instance_id: provider.base_instance_id.clone(),
            base_provider_generation: provider.provider_generation,
            resource_pool_id: resource_pool.clone(),
            resource_generation_id: resource_generation.clone(),
            operation_contract_id: operation.clone(),
            envelope_id: envelope.clone(),
            maximum_parameter_bytes: 1,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: operations,
        },
        scope: scope.clone(),
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 1,
        work_units: 1,
    };
    (
        conduit_std_host::isolated_copy_base::CopyBootstrap {
            issue,
            source_path: source.into(),
            destination_path: destination.into(),
            commit_policy: ProtectedResourceCommitPolicy::CreateOnly,
            maximum_bytes,
        },
        claim,
    )
}

#[test]
fn unchanged_copy_form_executes_through_isolated_provider_and_kernel() {
    let fixture = Fixture::new();
    let source_dir = fixture.directory("source");
    let destination_dir = fixture.directory("destination");
    let sibling_dir = fixture.directory("protected-sibling");
    let source = source_dir.join("input.bin");
    let destination = destination_dir.join("output.bin");
    let sibling = sibling_dir.join("sentinel.bin");
    let bytes = vec![0x5a; conduit_semantic_catalog::COPY_CHUNK_BYTES as usize + 17];
    std::fs::write(&source, &bytes).unwrap();
    std::fs::write(&sibling, b"protected").unwrap();

    let provider = IsolatedFileBaseConfig {
        executable: PathBuf::from(env!("CARGO_BIN_EXE_conduit-isolated-copy-base")),
        base_instance_id: conduit_core::BaseInstanceId::from("base/files/provider/one"),
        provider_generation: 1,
    };
    let mut host = conduit_std_host::isolated_copy_base::IsolatedFileHost::new(
        StdHostConfig {
            host_id: HostId::from("isolated-copy-host"),
            boot_id: BootId::from("isolated-copy-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        provider.clone(),
    )
    .unwrap();
    let offer = host
        .host()
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::COPY_FILE_KIND)
        .unwrap();
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        conduit_std_offers::ISOLATED_COPY_FILE_IMPLEMENTATION
    );

    let mut registry = ProtectedFileRegistry::default();
    let source_grant = register(
        &mut registry,
        &source,
        conduit_semantic_catalog::COPY_SOURCE_ROLE,
        ProtectedResourceAccess::ReadExisting,
        ProtectedResourceCommitPolicy::NotApplicable,
        bytes.len() as u64,
    );
    let destination_grant = register(
        &mut registry,
        &destination,
        conduit_semantic_catalog::COPY_DESTINATION_ROLE,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
        bytes.len() as u64,
    );
    let prepared = prepare_isolated_copy_task(
        host.host(),
        &[source_grant.clone(), destination_grant.clone()],
        &authority(),
    )
    .unwrap();
    assert!(!format!("{:?}", prepared.form).contains(source.to_string_lossy().as_ref()));
    assert!(!format!("{:?}", prepared.plan).contains(source.to_string_lossy().as_ref()));
    let proof_play = host
        .host_mut()
        .issue_kernel_play(&prepared.fragment)
        .unwrap();
    let (bootstrap, claim) = proof_material(
        &prepared.fragment,
        proof_play.identity(),
        &provider,
        &source_grant,
        &destination_grant,
        &source,
        &destination,
        bytes.len() as u64,
    );
    let proof = conduit_std_host::isolated_copy_base::run_adversarial_copy_proof(
        &provider.executable,
        bootstrap,
        claim,
        &sibling,
    )
    .unwrap();
    assert!(proof.forged_scope_refused);
    assert!(proof.stale_generation_refused_after_restart);
    assert_eq!(proof.sibling_read_errno, libc::EACCES);
    assert_eq!(proof.sibling_write_errno, libc::EACCES);
    assert_eq!(proof.process_spawn_errno, libc::EPERM);
    assert_eq!(std::fs::read(&sibling).unwrap(), b"protected");

    let play = host
        .host_mut()
        .issue_kernel_play(&prepared.fragment)
        .unwrap();
    let receipt = host
        .run_copy_fragment(
            play,
            CopyRequestId::new("isolated/create").unwrap(),
            prepared.fragment,
            &mut registry,
            &CopyStopToken::default(),
        )
        .unwrap();
    assert_eq!(
        receipt.result,
        CopyResult::Success {
            bytes_copied: bytes.len() as u64
        }
    );
    assert!(receipt.kernel_events > 0);
    assert!(receipt.presented_result.is_some());
    let inspection = host.inspection(&receipt);
    assert_eq!(
        inspection.enforcement_class,
        conduit_core::BaseEnforcementClass::ProcessIsolated
    );
    assert_eq!(inspection.provider_generation, 1);
    assert_eq!(inspection.attempt_id.as_str(), "isolated/create");
    assert_eq!(inspection.source_resource, source_grant.handle_id);
    assert_eq!(inspection.destination_resource, destination_grant.handle_id);
    let inspection_text = format!("{inspection:?}");
    assert!(!inspection_text.contains(source.to_string_lossy().as_ref()));
    assert!(!inspection_text.contains(destination.to_string_lossy().as_ref()));
    assert_eq!(std::fs::read(destination).unwrap(), bytes);
    assert_eq!(std::fs::read(sibling).unwrap(), b"protected");
}

#[test]
fn isolated_provider_preserves_replace_oversize_and_cancellation_semantics() {
    let fixture = Fixture::new();
    let source_dir = fixture.directory("source");
    let destination_dir = fixture.directory("destination");
    let source = source_dir.join("input.bin");
    let bytes = vec![0x2a; conduit_semantic_catalog::COPY_CHUNK_BYTES as usize + 9];
    std::fs::write(&source, &bytes).unwrap();

    let replacement = destination_dir.join("replacement.bin");
    std::fs::write(&replacement, b"old").unwrap();
    let receipt = run_isolated(
        &source,
        &replacement,
        ProtectedResourceCommitPolicy::ReplaceExisting,
        bytes.len() as u64,
        false,
        None,
    );
    assert_eq!(
        receipt.result,
        CopyResult::Success {
            bytes_copied: bytes.len() as u64
        }
    );
    assert_eq!(std::fs::read(&replacement).unwrap(), bytes);

    let oversized = destination_dir.join("oversized.bin");
    let receipt = run_isolated(
        &source,
        &oversized,
        ProtectedResourceCommitPolicy::CreateOnly,
        4,
        false,
        None,
    );
    assert_eq!(
        receipt.result,
        CopyResult::Oversized {
            source_bytes: bytes.len() as u64,
            maximum_bytes: 4
        }
    );
    assert!(!oversized.exists());

    let cancelled = destination_dir.join("cancelled.bin");
    let receipt = run_isolated(
        &source,
        &cancelled,
        ProtectedResourceCommitPolicy::CreateOnly,
        bytes.len() as u64,
        true,
        None,
    );
    assert_eq!(receipt.result, CopyResult::Cancelled { bytes_copied: 0 });
    assert!(!cancelled.exists());

    let failed = destination_dir.join("failed-provider.bin");
    let receipt = run_isolated(
        &source,
        &failed,
        ProtectedResourceCommitPolicy::CreateOnly,
        bytes.len() as u64,
        false,
        Some(Path::new("/bin/false")),
    );
    assert_eq!(receipt.result, CopyResult::Denied);
    assert!(!failed.exists());
}
