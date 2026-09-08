use super::base::{CopyFiles, ExecutionFaults};
use super::model::CopyResult;
use super::registry::ProtectedFileEntry;
use conduit_core::{ActivePlayIdentity, PlanFragment, ProtectedResourceCommitPolicy};
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
use conduit_core::{
    AuthorityGrant, BaseCapabilityAuthority, BaseCapabilityScope, BaseOperationClaim,
    CapabilityEnvelopeId, CapabilityIssueRequest, ResourceGenerationId, ResourcePoolId,
};

pub(super) enum CopyDriver {
    Cooperative(CopyFiles),
    #[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
    Isolated {
        client: crate::isolated_copy_base::IsolatedCopyClient,
        claim: Box<BaseOperationClaim>,
        bytes_copied: u64,
    },
}

impl CopyDriver {
    #[allow(clippy::too_many_arguments, clippy::needless_return)]
    pub(super) fn prepare(
        fragment: &PlanFragment,
        placement: &conduit_core::PlannedGear,
        active_play: &ActivePlayIdentity,
        source: &ProtectedFileEntry,
        destination: &ProtectedFileEntry,
        policy: ProtectedResourceCommitPolicy,
        maximum_bytes: u64,
        faults: ExecutionFaults,
        provider: Option<&crate::IsolatedFileBaseConfig>,
    ) -> Result<Self, CopyResult> {
        if placement.implementation_id.as_str()
            != conduit_std_offers::ISOLATED_COPY_FILE_IMPLEMENTATION
        {
            return CopyFiles::prepare(
                &source.path,
                &destination.path,
                policy,
                maximum_bytes,
                faults,
            )
            .map(Self::Cooperative);
        }
        #[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
        {
            let provider = provider.ok_or(CopyResult::Denied)?;
            let (issue, claim) = isolated_capability(
                fragment,
                placement,
                active_play,
                source,
                destination,
                provider,
                maximum_bytes,
            )?;
            let client = crate::isolated_copy_base::IsolatedCopyClient::start(
                &provider.executable,
                crate::isolated_copy_base::CopyBootstrap {
                    issue,
                    source_path: source.path.clone(),
                    destination_path: destination.path.clone(),
                    commit_policy: policy,
                    maximum_bytes,
                },
            )?;
            return Ok(Self::Isolated {
                client,
                claim: Box::new(claim),
                bytes_copied: 0,
            });
        }
        #[cfg(not(all(target_os = "linux", feature = "isolated-file-base")))]
        {
            let _ = (
                fragment,
                active_play,
                source,
                destination,
                provider,
                maximum_bytes,
            );
            Err(CopyResult::Denied)
        }
    }

    pub(super) fn step(&mut self) -> Result<bool, CopyResult> {
        match self {
            Self::Cooperative(files) => files.step(),
            #[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
            Self::Isolated {
                client,
                claim,
                bytes_copied,
            } => {
                let step = client.step(claim.as_ref().clone())?;
                *bytes_copied = step.bytes_copied;
                Ok(step.continues)
            }
        }
    }

    pub(super) fn bytes_copied(&self) -> u64 {
        match self {
            Self::Cooperative(files) => files.bytes_copied,
            #[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
            Self::Isolated { bytes_copied, .. } => *bytes_copied,
        }
    }

    pub(super) fn cancel(&mut self) -> CopyResult {
        match self {
            Self::Cooperative(files) => {
                if files.cleanup() {
                    CopyResult::Cancelled {
                        bytes_copied: files.bytes_copied,
                    }
                } else {
                    CopyResult::CleanupFailed {
                        bytes_copied: files.bytes_copied,
                    }
                }
            }
            #[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
            Self::Isolated { client, .. } => client.cancel(),
        }
    }
}

#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
#[allow(clippy::too_many_arguments)]
fn isolated_capability(
    fragment: &PlanFragment,
    placement: &conduit_core::PlannedGear,
    active_play: &ActivePlayIdentity,
    source: &ProtectedFileEntry,
    destination: &ProtectedFileEntry,
    provider: &crate::IsolatedFileBaseConfig,
    maximum_bytes: u64,
) -> Result<(CapabilityIssueRequest, BaseOperationClaim), CopyResult> {
    let authority = placement.authority.first().ok_or(CopyResult::Denied)?;
    if placement.authority.len() != 1 {
        return Err(CopyResult::Denied);
    }
    let operation = conduit_core::HostOperationContractId::from(
        conduit_std_offers::COPY_FILE_HOST_OPERATION_CONTRACT,
    );
    let resource_identity = format!(
        "{}+{}",
        source.grant.handle_id.as_str(),
        destination.grant.handle_id.as_str()
    );
    let resource_pool_id = ResourcePoolId::from(resource_identity.clone());
    let resource_generation_id = ResourceGenerationId(resource_identity);
    let envelope_id = CapabilityEnvelopeId::from(format!(
        "protected-file-pair/{}/{}/{}",
        source.grant.handle_id.as_str(),
        destination.grant.handle_id.as_str(),
        maximum_bytes
    ));
    let maximum_operations = maximum_bytes
        .div_ceil(conduit_semantic_catalog::COPY_CHUNK_BYTES as u64)
        .saturating_add(1)
        .min(u32::MAX as u64) as u32;
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: provider.base_instance_id.clone(),
        base_provider_generation: provider.provider_generation,
        plan_id: fragment.plan_id.clone(),
        active_play_id: active_play.active_play_id.clone(),
        authority_grant_id: authority.grant_id.clone(),
        authority_contract_id: authority.contract_id.clone(),
        capability_id: placement.capability_id.clone(),
        implementation_id: placement.implementation_id.clone(),
        operation_contract_id: operation.clone(),
        subject_kind: placement.kind_id.clone(),
        resource_pool_id: resource_pool_id.clone(),
        resource_generation_id: resource_generation_id.clone(),
        envelope_id: envelope_id.clone(),
        maximum_parameter_bytes: 1,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations,
    };
    let grant = AuthorityGrant {
        grant_id: authority.grant_id.clone(),
        contract_id: authority.contract_id.clone(),
        host_operation_contract_id: authority.host_operation_contract_id.clone(),
        subject_kind: authority.subject_kind.clone(),
        host_id: authority.host_id.clone(),
        boot_id: authority.boot_id.clone(),
        capability_id: authority.capability_id.clone(),
    };
    let issue = CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant,
            base_instance_id: provider.base_instance_id.clone(),
            base_provider_generation: provider.provider_generation,
            resource_pool_id: resource_pool_id.clone(),
            resource_generation_id: resource_generation_id.clone(),
            operation_contract_id: operation.clone(),
            envelope_id: envelope_id.clone(),
            maximum_parameter_bytes: 1,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations,
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
    Ok((issue, claim))
}
