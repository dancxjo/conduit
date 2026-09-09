use super::host::{capability, read, spawn_provider, write};
use super::{HttpBootstrap, IsolatedHttpBaseConfig, RequestFrame, ResponseFrame, PROTOCOL_VERSION};
use conduit_core::{BaseOperationClaim, CapabilityIssueRequest};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdversarialHttpReport {
    pub forged_scope_refused: bool,
    pub wrong_authority_refused_at_provider: bool,
    pub stale_generation_refused_after_restart: bool,
    pub revoked_handle_refused: bool,
    pub raw_socket_errno: i32,
    pub listener_errno: i32,
    pub process_spawn_errno: i32,
}

pub fn run_adversarial_http_proof(
    config: &IsolatedHttpBaseConfig,
    fragment: &conduit_core::PlanFragment,
    play: &crate::IssuedKernelPlay,
    exact_request: &conduit_web::HttpRequest,
    sibling_authority: &str,
) -> Result<AdversarialHttpReport, String> {
    let placement = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.implementation_id.as_str() == super::ISOLATED_HTTP_IMPLEMENTATION
        })
        .ok_or("adversarial Plan has no isolated HTTP placement")?;
    let (issue, claim) = capability(fragment, placement, play.identity(), config)?;

    let (
        forged_scope_refused,
        revoked_handle_refused,
        raw_socket_errno,
        listener_errno,
        process_spawn_errno,
    ) = revoked_and_os_probes(config, issue.clone(), claim.clone())?;

    let mut replacement = config.clone();
    replacement.provider_generation += 1;
    replacement.resource_generation_id = conduit_core::ResourceGenerationId(format!(
        "{}/replacement",
        config.resource_generation_id.0
    ));
    let (replacement_issue, _) = capability(fragment, placement, play.identity(), &replacement)?;
    let stale_generation_refused_after_restart =
        single_refusal(&replacement, replacement_issue, claim, exact_request)?
            == "capability:WrongScope";

    let mut wrong_request = exact_request.clone();
    wrong_request.target.authority = sibling_authority.into();
    let mut endpoint_check = replacement.clone();
    endpoint_check.provider_generation += 1;
    endpoint_check.resource_generation_id = conduit_core::ResourceGenerationId(format!(
        "{}/endpoint-check",
        config.resource_generation_id.0
    ));
    let (endpoint_issue, endpoint_claim) =
        capability(fragment, placement, play.identity(), &endpoint_check)?;
    let wrong_authority_refused_at_provider = single_refusal(
        &endpoint_check,
        endpoint_issue,
        endpoint_claim,
        &wrong_request,
    )? == "endpoint:wrong-authority";

    Ok(AdversarialHttpReport {
        forged_scope_refused,
        wrong_authority_refused_at_provider,
        stale_generation_refused_after_restart,
        revoked_handle_refused,
        raw_socket_errno,
        listener_errno,
        process_spawn_errno,
    })
}

fn revoked_and_os_probes(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
    claim: BaseOperationClaim,
) -> Result<(bool, bool, i32, i32, i32), String> {
    let (mut child, mut input, mut output) = start(config, issue)?;
    let mut forged = claim.clone();
    forged.subject_kind = conduit_core::KindId::from("http/forged-sibling");
    write(
        &mut input,
        &RequestFrame::Exchange {
            claim: Box::new(forged),
            request: Vec::new(),
        },
    )?;
    let forged_scope_refused = matches!(
        read(&mut output)?,
        ResponseFrame::Refused { reason } if reason == "capability:WrongScope"
    );
    let raw_socket_errno = probe(
        &mut input,
        &mut output,
        RequestFrame::ProbeRawSocket,
        "raw-socket",
    )?;
    let listener_errno = probe(
        &mut input,
        &mut output,
        RequestFrame::ProbeListener,
        "listener",
    )?;
    let process_spawn_errno = probe(
        &mut input,
        &mut output,
        RequestFrame::ProbeProcessSpawn,
        "process-spawn",
    )?;
    write(&mut input, &RequestFrame::Revoke)?;
    if read(&mut output)? != ResponseFrame::Revoked {
        return Err("HTTP proof provider did not revoke".into());
    }
    write(
        &mut input,
        &RequestFrame::Exchange {
            claim: Box::new(claim),
            request: Vec::new(),
        },
    )?;
    let revoked_handle_refused = matches!(
        read(&mut output)?,
        ResponseFrame::Refused { reason } if reason == "capability:Revoked"
    );
    stop(&mut child, input, output)?;
    Ok((
        forged_scope_refused,
        revoked_handle_refused,
        raw_socket_errno,
        listener_errno,
        process_spawn_errno,
    ))
}

fn single_refusal(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
    claim: BaseOperationClaim,
    request: &conduit_web::HttpRequest,
) -> Result<String, String> {
    let (mut child, mut input, mut output) = start(config, issue)?;
    write(
        &mut input,
        &RequestFrame::Exchange {
            claim: Box::new(claim),
            request: conduit_web::encode_request(request)
                .map_err(|_| "encode adversarial HTTP request")?,
        },
    )?;
    let reason = match read(&mut output)? {
        ResponseFrame::Refused { reason } => reason,
        other => return Err(format!("HTTP proof expected refusal, received {other:?}")),
    };
    stop(&mut child, input, output)?;
    Ok(reason)
}

type ProviderIo = (
    std::process::Child,
    std::process::ChildStdin,
    std::process::ChildStdout,
);

fn start(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
) -> Result<ProviderIo, String> {
    let stream = TcpStream::connect_timeout(&config.endpoint, Duration::from_secs(2))
        .map_err(|error| format!("connect exact HTTP proof endpoint: {error}"))?;
    let mut child =
        spawn_provider(&config.executable, &stream).map_err(|error| error.to_string())?;
    drop(stream);
    let mut input = child.stdin.take().ok_or("HTTP proof stdin missing")?;
    let mut output = child.stdout.take().ok_or("HTTP proof stdout missing")?;
    write(
        &mut input,
        &RequestFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            bootstrap: Box::new(HttpBootstrap {
                issue,
                expected_authority: config.authority.clone(),
                expected_endpoint: config.endpoint,
            }),
        },
    )?;
    match read(&mut output)? {
        ResponseFrame::Ready {
            enforcement,
            environment_entries: 0,
            descriptor_ceiling: 8,
            peer,
            ..
        } if enforcement == "os-capability/connected-fd+landlock+seccomp"
            && peer == config.endpoint => {}
        other => return Err(format!("HTTP proof provider not confined: {other:?}")),
    }
    Ok((child, input, output))
}

fn probe(
    input: &mut impl std::io::Write,
    output: &mut impl std::io::Read,
    request: RequestFrame,
    expected: &str,
) -> Result<i32, String> {
    write(input, &request)?;
    match read(output)? {
        ResponseFrame::ProbeDenied { probe, os_error } if probe == expected => Ok(os_error),
        other => Err(format!("HTTP proof {expected} was not denied: {other:?}")),
    }
}

fn stop(
    child: &mut std::process::Child,
    mut input: std::process::ChildStdin,
    mut output: std::process::ChildStdout,
) -> Result<(), String> {
    write(&mut input, &RequestFrame::Shutdown)?;
    if read(&mut output)? != ResponseFrame::Stopped {
        return Err("HTTP proof provider did not stop".into());
    }
    drop(input);
    if !child.wait().map_err(|error| error.to_string())?.success() {
        return Err("HTTP proof provider failed".into());
    }
    Ok(())
}
