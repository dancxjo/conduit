use super::{
    HttpBootstrap, RequestFrame, ResponseFrame, CONNECTED_SOCKET_FD, HTTP_CLIENT_OPERATION,
    HTTP_CLIENT_RESOURCE, ISOLATED_HTTP_ARTIFACT, ISOLATED_HTTP_IMPLEMENTATION,
    ISOLATED_HTTP_PROFILE, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use conduit_core::{
    ActivePlayIdentity, ArtifactId, AuthorityGrant, BaseCapabilityAuthority, BaseCapabilityScope,
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseOperationClaim,
    CapabilityEnvelopeId, CapabilityIssueRequest, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ResourceGenerationId,
};
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

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
    provider: IsolatedHttpBaseConfig,
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
    let mut offer = crate::installed_std::http_client_offer();
    offer.capability_id = conduit_core::CapabilityId::from("std-isolated-http-client-http1");
    offer.implementation.execution_profile_id = ExecutionProfileId::from(ISOLATED_HTTP_PROFILE);
    offer.implementation.implementation_id = ImplementationId::from(ISOLATED_HTTP_IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(ISOLATED_HTTP_ARTIFACT);
    offer
}

impl IsolatedHttpHost {
    pub fn new(
        config: crate::StdHostConfig,
        mut composition: crate::StdHostComposition,
        provider: IsolatedHttpBaseConfig,
    ) -> Result<Self, String> {
        provider.validate()?;
        composition.http = true;
        let mut host = crate::StdHost::new_with_composition(config, composition);
        host.advertisement
            .capabilities
            .retain(|offer| offer.kind_id.as_str() != conduit_web::HTTP_CLIENT_KIND);
        host.advertisement
            .capabilities
            .push(isolated_http_client_offer());
        host.advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)
                .map_err(|error| format!("isolated HTTP Base resources: {error}"))?;
        Ok(Self { host, provider })
    }

    pub fn host(&self) -> &crate::StdHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut crate::StdHost {
        &mut self.host
    }

    pub fn exchange(
        &mut self,
        play: crate::IssuedKernelPlay,
        fragment: &conduit_core::PlanFragment,
        request: &conduit_web::HttpRequest,
    ) -> Result<IsolatedHttpExchangeReceipt, String> {
        let active_play = play.identity();
        if active_play.plan_id != fragment.plan_id
            || active_play.host_id != fragment.host_id
            || active_play.boot_id != fragment.boot_id
        {
            return Err("HTTP Play identity does not match its immutable Plan fragment".into());
        }
        request
            .validate()
            .map_err(|_| "isolated HTTP request exceeds its semantic bounds".to_string())?;
        if request.target.scheme != "http" || request.target.authority != self.provider.authority {
            return Err("isolated HTTP request is outside the selected exact endpoint".into());
        }
        let placement = exact_http_placement(fragment)?;
        let (issue, claim) = capability(fragment, placement, active_play, &self.provider)?;
        let reservation = self
            .host
            .kernel_resources
            .prepare_and_reserve(&self.host.advertisement, fragment)?;
        let result = exchange_provider(&self.provider, issue, claim, request);
        let release = self.host.kernel_resources.release(reservation);
        release?;
        let response = result?;
        Ok(IsolatedHttpExchangeReceipt {
            active_play_id: active_play.active_play_id.clone(),
            status: response.status,
            response_bytes: conduit_web::encode_response(&response)
                .map_err(|_| "isolated HTTP response exceeds semantic bounds")?
                .len() as u32,
        })
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

fn exact_http_placement(
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
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != HTTP_CLIENT_OPERATION
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
    let authority = &placement.authority[0];
    let resource = &placement.resources[0];
    let operation = placement.host_operations[0].clone();
    let envelope_id = CapabilityEnvelopeId::from(format!(
        "http/exact-endpoint/{}/max-{}/{}",
        provider.resource_generation_id.0,
        operation.maximum_input_bytes,
        operation.maximum_output_bytes
    ));
    let scope = BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: provider.base_instance_id.clone(),
        base_provider_generation: provider.provider_generation,
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
        host_operation_contract_id: authority.host_operation_contract_id.clone(),
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

fn exchange_provider(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
    mut claim: BaseOperationClaim,
    request: &conduit_web::HttpRequest,
) -> Result<conduit_web::HttpResponse, String> {
    let encoded = conduit_web::encode_request(request)
        .map_err(|_| "encode bounded isolated HTTP request".to_string())?;
    claim.parameter_bytes = encoded.len() as u32;
    let stream = TcpStream::connect_timeout(&config.endpoint, Duration::from_secs(2))
        .map_err(|error| format!("connect exact HTTP endpoint: {error}"))?;
    if stream.peer_addr().map_err(|error| error.to_string())? != config.endpoint {
        return Err("connected HTTP descriptor has the wrong exact peer".into());
    }
    let mut child =
        spawn_provider(&config.executable, &stream).map_err(|error| error.to_string())?;
    drop(stream);
    let mut input = child
        .stdin
        .take()
        .ok_or("isolated HTTP provider stdin missing")?;
    let mut output = child
        .stdout
        .take()
        .ok_or("isolated HTTP provider stdout missing")?;
    crate::isolated_base::write_frame_with_limit(
        &mut input,
        &RequestFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            bootstrap: Box::new(HttpBootstrap {
                issue,
                expected_authority: config.authority.clone(),
                expected_endpoint: config.endpoint,
            }),
        },
        MAX_FRAME_BYTES,
    )
    .map_err(|error| error.to_string())?;
    match read(&mut output)? {
        ResponseFrame::Ready { peer, .. } if peer == config.endpoint => {}
        other => return Err(format!("isolated HTTP provider not ready: {other:?}")),
    }
    write(
        &mut input,
        &RequestFrame::Exchange {
            claim: Box::new(claim),
            request: encoded,
        },
    )?;
    let response = match read(&mut output)? {
        ResponseFrame::Completed { response } => conduit_web::decode_response(&response)
            .map_err(|_| "isolated HTTP provider returned malformed response".to_string())?,
        ResponseFrame::Refused { reason } => {
            return Err(format!("isolated HTTP refused: {reason}"))
        }
        other => return Err(format!("isolated HTTP provider returned {other:?}")),
    };
    write(&mut input, &RequestFrame::Shutdown)?;
    if read(&mut output)? != ResponseFrame::Stopped {
        return Err("isolated HTTP provider did not stop".into());
    }
    drop(input);
    if !child.wait().map_err(|error| error.to_string())?.success() {
        return Err("isolated HTTP provider failed".into());
    }
    Ok(response)
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
