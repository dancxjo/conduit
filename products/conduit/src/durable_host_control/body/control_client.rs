//! One finite local control round trip. A lost reply is an unknown outcome:
//! the owner may already have accepted and applied the request.

use std::{
    io::{Read, Write},
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::Path,
    time::{Duration, Instant},
};

use socket2::{Domain, SockAddr, Socket, Type};

use super::super::{Request, Response, CONTROL_OUTCOME_UNKNOWN, MAXIMUM_CONTROL_FRAME_BYTES};

const CONTROL_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) fn call(state_dir: &Path, mut request: Request) -> Result<Response, String> {
    let mut bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("encode local control request: {error}"))?;
    clear_token(&mut request);
    if bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("local control request violates its finite bound".into());
    }
    let result = round_trip(&state_dir.join("control.sock"), &bytes);
    bytes.fill(0);
    result
}

fn round_trip(path: &Path, bytes: &[u8]) -> Result<Response, String> {
    let deadline = Instant::now() + CONTROL_DEADLINE;
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
        .map_err(|error| format!("open local control socket: {error}"))?;
    let address =
        SockAddr::unix(path).map_err(|error| format!("address local control socket: {error}"))?;
    socket
        .connect_timeout(&address, remaining(deadline)?)
        .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_owned())?;
    let mut stream = UnixStream::from(OwnedFd::from(socket));
    let mut sent = 0;
    while sent < bytes.len() {
        stream
            .set_write_timeout(Some(remaining(deadline)?))
            .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_owned())?;
        match stream.write(&bytes[sent..]) {
            Ok(0) => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
            Ok(count) => sent += count,
            Err(_) => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
        }
    }
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_owned())?;
    let mut response = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];
    loop {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_owned())?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                if response.len().saturating_add(count) > MAXIMUM_CONTROL_FRAME_BYTES {
                    return Err(CONTROL_OUTCOME_UNKNOWN.into());
                }
                response.extend_from_slice(&chunk[..count]);
            }
            Err(_) => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
        }
    }
    serde_json::from_slice(&response).map_err(|_| CONTROL_OUTCOME_UNKNOWN.into())
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| CONTROL_OUTCOME_UNKNOWN.into())
}

fn clear_token(request: &mut Request) {
    match request {
        Request::BodyInspect { token, .. }
        | Request::BodyInvite { token, .. }
        | Request::BodyAdmit { token, .. }
        | Request::BodyBrowserStart { token, .. }
        | Request::BodyFace { token, .. }
        | Request::BodyLocalFace { token, .. }
        | Request::BirthFace { token, .. }
        | Request::BirthInteraction { token, .. }
        | Request::BodyInteraction { token, .. }
        | Request::BodyAttachedTerminalInteraction { token, .. }
        | Request::BodyBrowserInteraction { token, .. }
        | Request::BodyStart { token, .. }
        | Request::BodyLull { token, .. } => token.fill(0),
        _ => unreachable!("Body control client only sends Body and Birth requests"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::net::UnixListener,
        sync::mpsc,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn accepted_request_with_no_reply_has_an_unknown_outcome() {
        let directory = std::env::temp_dir().join(format!(
            "conduit-control-timeout-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let (finished, release) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).unwrap();
            assert_eq!(request, b"{}");
            release.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        assert!(matches!(round_trip(&path, b"{}"), Err(code) if code == CONTROL_OUTCOME_UNKNOWN));
        finished.send(()).unwrap();
        server.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
}
