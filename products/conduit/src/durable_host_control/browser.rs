//! Short authenticated owner operations used by the loopback browser worker.
//! Network waits remain on that worker; this control carrier never decides
//! admission, membership, authority, or elapsed authorization time.
use super::{read_frame, read_secret, write_frame, Request, Response, PROTOCOL};
use crate::durable_host::owner::{BrowserAdmittedSnapshot, BrowserWindowAuthorization};
use conduit_body::{BodyBiographyEvidence, MembershipCredential};
use conduit_core::LinkBindingId;
use conduit_std_host::browser_admission::{BrowserAdmissionEgress, BrowserAdmissionIngress};
use std::path::Path;

pub(crate) fn spawn_window(
    state_dir: &Path,
    authorization: BrowserWindowAuthorization,
) -> Result<String, String> {
    crate::durable_host::owner::run_service_window(state_dir, authorization)
}

fn call(state_dir: &Path, request: impl FnOnce(Vec<u8>) -> Request) -> Result<Response, String> {
    use std::os::unix::net::UnixStream;
    let mut secret = read_secret(&state_dir.join("control.token"))?;
    let mut request = request(secret.to_vec());
    secret.fill(0);
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to current Body owner service: {error}"))?;
    let sent = write_frame(&mut stream, &request);
    match &mut request {
        Request::BodyBrowserBegin { token, .. }
        | Request::BodyBrowserComplete { token, .. }
        | Request::BodyBrowserAbort { token, .. }
        | Request::BodyBrowserLeave { token, .. }
        | Request::BodyBrowserCancel { token, .. } => token.fill(0),
        _ => unreachable!("browser worker only sends browser owner operations"),
    }
    sent?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish browser owner control request: {error}"))?;
    read_frame(&mut stream)
}

pub(crate) fn begin(
    state_dir: &Path,
    window_id: &str,
    binding: LinkBindingId,
    frame: BrowserAdmissionIngress,
    encoded_bytes: u32,
) -> Result<BrowserAdmissionEgress, String> {
    match call(state_dir, |token| Request::BodyBrowserBegin {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
        binding,
        frame: Box::new(frame),
        encoded_bytes,
    })? {
        Response::BodyBrowserChallenge {
            protocol: PROTOCOL,
            frame,
        } => Ok(*frame),
        Response::Refused { code, .. } => {
            Err(format!("Body owner refused browser challenge: {code}"))
        }
        _ => Err("Body owner returned the wrong browser challenge response".into()),
    }
}

pub(crate) fn complete(
    state_dir: &Path,
    window_id: &str,
    frame: BrowserAdmissionIngress,
) -> Result<BrowserAdmittedSnapshot, String> {
    match call(state_dir, |token| Request::BodyBrowserComplete {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
        frame: Box::new(frame),
    })? {
        Response::BodyBrowserSnapshot {
            protocol: PROTOCOL,
            snapshot,
        } => Ok(*snapshot),
        Response::Refused { code, .. } => Err(format!("Body owner refused browser proof: {code}")),
        _ => Err("Body owner returned the wrong browser proof response".into()),
    }
}

pub(crate) fn abort(state_dir: &Path, window_id: &str) -> Result<(), String> {
    match call(state_dir, |token| Request::BodyBrowserAbort {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
    })? {
        Response::BodyBrowserAborted { protocol: PROTOCOL } => Ok(()),
        Response::Refused { code, .. } => Err(format!("Body owner refused browser abort: {code}")),
        _ => Err("Body owner returned the wrong browser abort response".into()),
    }
}

pub(crate) fn leave(
    state_dir: &Path,
    window_id: &str,
    credential: MembershipCredential,
) -> Result<BodyBiographyEvidence, String> {
    match call(state_dir, |token| Request::BodyBrowserLeave {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
        credential,
    })? {
        Response::BodyBrowserLeft {
            protocol: PROTOCOL,
            biography,
        } => Ok(*biography),
        Response::Refused { code, .. } => Err(format!("Body owner refused browser leave: {code}")),
        _ => Err("Body owner returned the wrong browser leave response".into()),
    }
}

pub(crate) fn cancel(state_dir: &Path, window_id: &str) -> Result<(), String> {
    match call(state_dir, |token| Request::BodyBrowserCancel {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
    })? {
        Response::BodyBrowserCancelled { protocol: PROTOCOL } => Ok(()),
        Response::Refused { code, .. } => {
            Err(format!("Body owner refused browser window cleanup: {code}"))
        }
        _ => Err("Body owner returned the wrong browser window cleanup response".into()),
    }
}
