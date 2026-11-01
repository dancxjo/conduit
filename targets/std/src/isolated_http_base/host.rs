use super::{
    RequestFrame, ResponseFrame, CONNECTED_SOCKET_FD, HTTP_CLIENT_OPERATION, HTTP_CLIENT_RESOURCE,
    ISOLATED_HTTP_ARTIFACT, ISOLATED_HTTP_IMPLEMENTATION, ISOLATED_HTTP_PROFILE, MAX_FRAME_BYTES,
};
use conduit_core::{
    ActivePlayIdentity, AuthorityGrant, BaseCapabilityAuthority, BaseCapabilityScope,
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle, BaseOperationClaim,
    BaseProviderEntry, BaseRegistry, BaseRegistryLimits, CapabilityEnvelopeId,
    CapabilityIssueRequest, CapabilityOffer, HostBaseId, HostBaseKindId, ResourceGenerationId,
};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct IsolatedHttpBaseConfig {
    pub executable: PathBuf,
    pub base_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub authority: String,
    pub endpoint: SocketAddr,
    pub resource_generation_id: ResourceGenerationId,
}

impl IsolatedHttpBaseConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.executable.as_os_str().is_empty()
            || self.provider_generation == 0
            || self.authority.is_empty()
            || self.endpoint.port() == 0
        {
            return Err("isolated HTTP Base configuration is incomplete".into());
        }
        Ok(())
    }
}

pub struct IsolatedHttpHost {
    host: crate::StdHost,
    registry: BaseRegistry,
    provider: IsolatedHttpBaseConfig,
    incoming: conduit_core::HostAdvertisement,
    incoming_registry: BaseRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedHttpExchangeReceipt {
    pub active_play_id: conduit_core::ActivePlayId,
    pub status: u16,
    pub response_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedHttpBaseInspection {
    pub base_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub implementation_id: BaseImplementationId,
    pub enforcement_class: BaseEnforcementClass,
    pub authority: String,
    pub endpoint: SocketAddr,
    pub resource_generation_id: ResourceGenerationId,
}

pub fn isolated_http_client_offer() -> CapabilityOffer {
    crate::installed_std::http_client_offer_for(
        "std-isolated-http-client-http1",
        ISOLATED_HTTP_PROFILE,
        ISOLATED_HTTP_IMPLEMENTATION,
        ISOLATED_HTTP_ARTIFACT,
    )
}

impl IsolatedHttpHost {
    pub fn new(
        config: crate::StdHostConfig,
        mut composition: crate::StdHostComposition,
        provider: IsolatedHttpBaseConfig,
    ) -> Result<Self, String> {
        provider.validate()?;
        composition.http = true;
        Self::from_host(
            crate::StdHost::new_with_composition(config, composition),
            provider,
        )
        .map_err(|(_, error)| error)
    }

    /// Transfer the actual idle installed Host; errors return that same owner.
    #[allow(clippy::result_large_err)]
    pub fn from_host(
        mut host: crate::StdHost,
        provider: IsolatedHttpBaseConfig,
    ) -> Result<Self, (crate::StdHost, String)> {
        if !host.kernel_resources.is_idle() {
            return Err((
                host,
                "HTTP installation requires idle actual Host resources".into(),
            ));
        }
        if let Err(error) = provider.validate() {
            return Err((host, error));
        }
        let original = host.advertisement.clone();
        let prepared = (|| -> Result<BaseRegistry, String> {
            host.advertisement
                .bases
                .retain(|base| base.implementation_id.as_str() != ISOLATED_HTTP_IMPLEMENTATION);
            host.advertisement.capabilities.retain(|offer| {
                !matches!(
                    offer.capability_id.as_str(),
                    "std-http-client-http1" | "std-isolated-http-client-http1"
                )
            });
            let resource_index = host
                .advertisement
                .resources
                .iter()
                .position(|offer| {
                    offer.class_id.as_str() == HTTP_CLIENT_RESOURCE
                        && !host
                            .advertisement
                            .bases
                            .iter()
                            .any(|base| base.resource_pool_ids.contains(&offer.pool_id))
                })
                .ok_or("std Host has no HTTP client resource for the isolated Base")?;
            let resource = host.advertisement.resources.remove(resource_index);
            let maximum_advertised_capabilities =
                u16::try_from(host.advertisement.capabilities.len().saturating_add(1))
                    .map_err(|_| "std Host capability inventory exceeds Base registry bounds")?;
            let maximum_advertised_resources =
                u16::try_from(host.advertisement.resources.len().saturating_add(1))
                    .map_err(|_| "std Host resource inventory exceeds Base registry bounds")?;
            let mut registry = BaseRegistry::new(BaseRegistryLimits {
                maximum_bases: 1,
                maximum_capabilities_per_base: 1,
                maximum_resources_per_base: 1,
                maximum_advertised_capabilities,
                maximum_advertised_resources,
            })
            .map_err(|error| format!("isolated HTTP Base registry: {error:?}"))?;
            registry
                .register(BaseProviderEntry {
                    base_id: HostBaseId::from("std/base/isolated-http"),
                    provider_instance_id: provider.base_instance_id.clone(),
                    provider_generation: provider.provider_generation,
                    implementation_id: BaseImplementationId::from(ISOLATED_HTTP_IMPLEMENTATION),
                    mechanism_family: HostBaseKindId::from("conduit.base/http-client@1"),
                    enforcement_class: BaseEnforcementClass::OsCapabilityMediated,
                    lifecycle: BaseLifecycle::Ready,
                    capabilities: vec![isolated_http_client_offer()],
                    resources: vec![resource],
                })
                .map_err(|error| format!("isolated HTTP Base registration: {error:?}"))?;
            registry
                .project_ready_into(&mut host.advertisement)
                .map_err(|error| format!("isolated HTTP Base advertisement: {error:?}"))?;
            host.advertisement.capabilities.sort_by(|left, right| {
                left.capability_id
                    .as_str()
                    .cmp(right.capability_id.as_str())
            });
            host.advertisement
                .resources
                .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
            host.kernel_resources =
                crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)
                    .map_err(|error| format!("isolated HTTP Base resources: {error}"))?;
            Ok(registry)
        })();
        match prepared {
            Ok(registry) => {
                let incoming_registry =
                    std::mem::replace(&mut host.base_registry, registry.clone());
                Ok(Self {
                    host,
                    registry,
                    provider,
                    incoming: original,
                    incoming_registry,
                })
            }
            Err(error) => {
                host.advertisement = original;
                host.kernel_resources =
                    crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)
                        .expect("previous valid idle resource ledger");
                Err((host, error))
            }
        }
    }
    pub fn into_host(mut self) -> crate::StdHost {
        assert!(
            self.host.kernel_resources.is_idle(),
            "temporary HTTP owner returned with active resources"
        );
        self.host.advertisement = self.incoming;
        self.host.base_registry = self.incoming_registry;
        self.host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&self.host.advertisement)
                .expect("retained incoming valid inventory");
        self.host
    }

    pub fn host(&self) -> &crate::StdHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut crate::StdHost {
        &mut self.host
    }

    pub fn registry(&self) -> &BaseRegistry {
        &self.registry
    }

    pub fn exchange(
        &mut self,
        play: crate::IssuedKernelPlay,
        fragment: &conduit_core::PlanFragment,
        request: &conduit_web::HttpRequest,
    ) -> Result<IsolatedHttpExchangeReceipt, String> {
        self.exchange_response(play, fragment, request)
            .map(|(receipt, _)| receipt)
    }

    /// Preserve the bounded typed response produced by the selected confined Base.
    pub fn exchange_response(
        &mut self,
        play: crate::IssuedKernelPlay,
        fragment: &conduit_core::PlanFragment,
        request: &conduit_web::HttpRequest,
    ) -> Result<(IsolatedHttpExchangeReceipt, conduit_web::HttpResponse), String> {
        super::exchange::exchange_with_registry(
            &mut self.host,
            &self.registry,
            &self.provider,
            play,
            fragment,
            request,
        )
    }

    pub fn inspection(&self) -> IsolatedHttpBaseInspection {
        IsolatedHttpBaseInspection {
            base_instance_id: self.provider.base_instance_id.clone(),
            provider_generation: self.provider.provider_generation,
            implementation_id: BaseImplementationId::from(ISOLATED_HTTP_IMPLEMENTATION),
            enforcement_class: BaseEnforcementClass::OsCapabilityMediated,
            authority: self.provider.authority.clone(),
            endpoint: self.provider.endpoint,
            resource_generation_id: self.provider.resource_generation_id.clone(),
        }
    }
}

pub(super) fn exact_http_placement(
    fragment: &conduit_core::PlanFragment,
) -> Result<&conduit_core::PlannedGear, String> {
    let mut placements = fragment
        .placements
        .iter()
        .filter(|placement| placement.implementation_id.as_str() == ISOLATED_HTTP_IMPLEMENTATION);
    let placement = placements
        .next()
        .ok_or("Plan has no isolated HTTP client placement")?;
    if placements.next().is_some()
        || placement.kind_id.as_str() != conduit_web::HTTP_CLIENT_KIND
        || placement.host_calls.len() != 1
        || placement.host_calls[0].contract_id.as_str() != HTTP_CLIENT_OPERATION
        || placement.authority.len() != 1
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != HTTP_CLIENT_RESOURCE
    {
        return Err("planned HTTP realization does not match the exact isolated profile".into());
    }
    Ok(placement)
}

pub(super) fn capability(
    fragment: &conduit_core::PlanFragment,
    placement: &conduit_core::PlannedGear,
    play: &ActivePlayIdentity,
    provider: &IsolatedHttpBaseConfig,
) -> Result<(CapabilityIssueRequest, BaseOperationClaim), String> {
    capability_for_installed_base(
        fragment,
        placement,
        play,
        provider,
        ISOLATED_HTTP_IMPLEMENTATION,
    )
}

pub(super) fn capability_for_installed_base(
    fragment: &conduit_core::PlanFragment,
    placement: &conduit_core::PlannedGear,
    play: &ActivePlayIdentity,
    provider: &IsolatedHttpBaseConfig,
    installed_implementation: &str,
) -> Result<(CapabilityIssueRequest, BaseOperationClaim), String> {
    let authority = &placement.authority[0];
    let resource = &placement.resources[0];
    let operation = placement.host_calls[0].clone();
    let base = placement
        .base
        .as_ref()
        .ok_or("planned HTTP realization has no Base provider provenance")?;
    if base.provider_instance_id != provider.base_instance_id
        || base.provider_generation != provider.provider_generation
        || base.implementation_id.as_str() != installed_implementation
    {
        return Err("planned HTTP Base provider is stale or does not match the executor".into());
    }
    let envelope_id = CapabilityEnvelopeId::from(format!(
        "http/exact-endpoint/{}/max-{}/{}",
        provider.resource_generation_id.0,
        operation.maximum_input_bytes,
        operation.maximum_output_bytes
    ));
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: base.provider_instance_id.clone(),
        base_provider_generation: base.provider_generation,
        plan_id: fragment.plan_id.clone(),
        active_play_id: play.active_play_id.clone(),
        authority_grant_id: authority.grant_id.clone(),
        authority_contract_id: authority.contract_id.clone(),
        capability_id: placement.capability_id.clone(),
        implementation_id: placement.implementation_id.clone(),
        operation_contract_id: operation.contract_id.clone(),
        subject_kind: authority.subject_kind.clone(),
        resource_pool_id: resource.pool_id.clone(),
        resource_generation_id: provider.resource_generation_id.clone(),
        envelope_id: envelope_id.clone(),
        maximum_parameter_bytes: operation.maximum_input_bytes,
        maximum_result_bytes: operation.maximum_output_bytes,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    let grant = AuthorityGrant {
        grant_id: authority.grant_id.clone(),
        contract_id: authority.contract_id.clone(),
        host_call_contract_id: authority.host_call_contract_id.clone(),
        subject_kind: authority.subject_kind.clone(),
        host_id: authority.host_id.clone(),
        boot_id: authority.boot_id.clone(),
        capability_id: authority.capability_id.clone(),
    };
    let issue = CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant,
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: scope.maximum_parameter_bytes,
            maximum_result_bytes: scope.maximum_result_bytes,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
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
        parameter_bytes: operation.maximum_input_bytes,
        work_units: 1,
    };
    Ok((issue, claim))
}

pub(super) fn exchange_provider(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
    claim: BaseOperationClaim,
    request: &conduit_web::HttpRequest,
) -> Result<conduit_web::HttpResponse, String> {
    super::provider_exchange::exchange_provider_classified(config, issue, claim, request)
        .map_err(String::from)
}

pub(super) fn spawn_provider(
    executable: &Path,
    stream: &TcpStream,
) -> io::Result<std::process::Child> {
    let socket_fd = stream.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if socket_fd != CONNECTED_SOCKET_FD {
                if libc::dup2(socket_fd, CONNECTED_SOCKET_FD) < 0 {
                    return Err(io::Error::last_os_error());
                }
            } else if libc::fcntl(CONNECTED_SOCKET_FD, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            set_limit(libc::RLIMIT_AS, 128 * 1024 * 1024)?;
            set_limit(libc::RLIMIT_CPU, 2)?;
            set_limit(libc::RLIMIT_NOFILE, 8)?;
            set_limit(libc::RLIMIT_NPROC, 1)?;
            set_limit(libc::RLIMIT_FSIZE, 1024 * 1024)?;
            Ok(())
        });
    }
    command.spawn()
}

fn set_limit(resource: libc::__rlimit_resource_t, value: libc::rlim_t) -> io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    if unsafe { libc::setrlimit(resource, &limit) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn write(writer: &mut impl io::Write, frame: &RequestFrame) -> Result<(), String> {
    crate::isolated_base::write_frame_with_limit(writer, frame, MAX_FRAME_BYTES)
        .map_err(|error| error.to_string())
}

pub(super) fn read(reader: &mut impl io::Read) -> Result<ResponseFrame, String> {
    crate::isolated_base::read_frame_with_limit(reader, MAX_FRAME_BYTES)
        .map_err(|error| error.to_string())
}
