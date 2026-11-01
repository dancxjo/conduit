//! One exact confined exchange; finite diagnostic categories never expose worker text.
use super::host::{read, spawn_provider, write};
use super::{
    HttpBootstrap, IsolatedHttpBaseConfig, RequestFrame, ResponseFrame, MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
};
use conduit_core::{BaseOperationClaim, CapabilityIssueRequest};
use std::{net::TcpStream, time::Duration};
pub(super) fn exchange_provider_classified(
    config: &IsolatedHttpBaseConfig,
    issue: CapabilityIssueRequest,
    mut claim: BaseOperationClaim,
    request: &conduit_web::HttpRequest,
) -> Result<conduit_web::HttpResponse, super::HttpExchangeRefusal> {
    use super::HttpExchangeRefusal as F;
    let encoded = conduit_web::encode_request(request).map_err(|_| F::Capacity)?;
    claim.parameter_bytes = encoded.len() as u32;
    let stream =
        TcpStream::connect_timeout(&config.endpoint, Duration::from_secs(2)).map_err(F::io)?;
    if stream.peer_addr().map_err(F::io)? != config.endpoint {
        return Err(F::Authorization);
    }
    let mut child = super::child::ProviderChild::new(
        spawn_provider(&config.executable, &stream).map_err(|_| F::ProviderSpawn)?,
    );
    drop(stream);
    let mut input = child.stdin.take().ok_or(F::ProviderProtocol)?;
    let mut output = child.stdout.take().ok_or(F::ProviderProtocol)?;
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
    .map_err(|_| F::ProviderProtocol)?;
    match read(&mut output).map_err(|_| F::ProviderProtocol)? {
        ResponseFrame::Ready { peer, .. } if peer == config.endpoint => {}
        _ => return Err(F::ProviderProtocol),
    }
    write(
        &mut input,
        &RequestFrame::Exchange {
            claim: Box::new(claim),
            request: encoded,
        },
    )
    .map_err(|_| F::ProviderProtocol)?;
    let response = match read(&mut output).map_err(|_| F::ProviderProtocol)? {
        ResponseFrame::Completed { response } => {
            conduit_web::decode_response(&response).map_err(|_| F::MalformedResponse)?
        }
        ResponseFrame::Refused { reason } => return Err(F::provider(&reason)),
        _ => return Err(F::ProviderProtocol),
    };
    write(&mut input, &RequestFrame::Shutdown).map_err(|_| F::ProviderProtocol)?;
    if read(&mut output).map_err(|_| F::ProviderProtocol)? != ResponseFrame::Stopped {
        return Err(F::ProviderProtocol);
    }
    drop(input);
    if !child.wait().map_err(|_| F::ProviderProtocol)?.success() {
        return Err(F::ProviderProtocol);
    }
    Ok(response)
}
