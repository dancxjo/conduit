use super::{
    HttpBootstrap, RequestFrame, ResponseFrame, CONNECTED_SOCKET_FD, HTTP_CLIENT_OPERATION,
    MAX_FRAME_BYTES, PROTOCOL_VERSION,
};
use conduit_core::{BaseCapabilityTable, BaseOperationClaim};
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::FromRawFd;
use std::process::Command;
use std::time::Duration;

pub fn provider_main() -> Result<(), String> {
    let request: RequestFrame =
        crate::isolated_base::read_frame_with_limit(&mut io::stdin(), MAX_FRAME_BYTES)
            .map_err(io_error)?;
    let RequestFrame::Bootstrap {
        protocol_version,
        bootstrap,
    } = request
    else {
        return Err("first HTTP Base frame was not bootstrap".into());
    };
    if protocol_version != PROTOCOL_VERSION {
        return Err("HTTP Base protocol version mismatch".into());
    }
    validate_bootstrap(&bootstrap)?;
    let mut stream = unsafe { TcpStream::from_raw_fd(CONNECTED_SOCKET_FD) };
    let peer = stream.peer_addr().map_err(io_error)?;
    if peer != bootstrap.expected_endpoint {
        return Err("inherited HTTP descriptor has the wrong peer".into());
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(2))))
        .map_err(io_error)?;

    let mut issuer_key = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut issuer_key))
        .map_err(io_error)?;
    let mut table = BaseCapabilityTable::new(
        bootstrap.issue.scope.host_id.clone(),
        bootstrap.issue.scope.boot_id.clone(),
        bootstrap.issue.scope.base_instance_id.clone(),
        bootstrap.issue.scope.base_provider_generation,
        issuer_key,
        1,
    )
    .map_err(capability_error)?;
    let handle = table
        .issue(bootstrap.issue.clone())
        .map_err(capability_error)?;

    apply_empty_landlock()?;
    crate::isolated_base::apply_seccomp()?;
    write(&ResponseFrame::Ready {
        protocol_version: PROTOCOL_VERSION,
        enforcement: "os-capability/connected-fd+landlock+seccomp".into(),
        environment_entries: std::env::vars_os().count() as u16,
        descriptor_ceiling: descriptor_ceiling()?,
        peer,
    })?;

    for _ in 0..8 {
        match crate::isolated_base::read_frame_with_limit::<_, RequestFrame>(
            &mut io::stdin(),
            MAX_FRAME_BYTES,
        )
        .map_err(io_error)?
        {
            RequestFrame::Exchange { claim, request } => {
                exchange(
                    &mut table,
                    &handle,
                    &bootstrap,
                    &mut stream,
                    &claim,
                    &request,
                )?;
            }
            RequestFrame::Revoke => {
                table.revoke(&handle).map_err(capability_error)?;
                write(&ResponseFrame::Revoked)?;
            }
            RequestFrame::ProbeRawSocket => probe_socket()?,
            RequestFrame::ProbeListener => probe_listener()?,
            RequestFrame::ProbeProcessSpawn => probe_process()?,
            RequestFrame::Shutdown => {
                write(&ResponseFrame::Stopped)?;
                return Ok(());
            }
            RequestFrame::Bootstrap { .. } => return Err("duplicate HTTP Base bootstrap".into()),
        }
    }
    Err("HTTP Base operation bound exhausted".into())
}

fn exchange(
    table: &mut BaseCapabilityTable,
    handle: &conduit_core::BaseCapabilityHandle,
    bootstrap: &HttpBootstrap,
    stream: &mut TcpStream,
    claim: &BaseOperationClaim,
    encoded: &[u8],
) -> Result<(), String> {
    let lease = match table.authorize(handle, claim) {
        Ok(lease) => lease,
        Err(refusal) => {
            return write(&ResponseFrame::Refused {
                reason: format!("capability:{refusal:?}"),
            });
        }
    };
    let request = match conduit_web::decode_request(encoded) {
        Ok(request) => request,
        Err(_) => {
            table.complete(lease, 0).map_err(capability_error)?;
            return write(&ResponseFrame::Refused {
                reason: "request:malformed-or-oversized".into(),
            });
        }
    };
    if request.target.scheme != "http" || request.target.authority != bootstrap.expected_authority {
        table.complete(lease, 0).map_err(capability_error)?;
        return write(&ResponseFrame::Refused {
            reason: "endpoint:wrong-authority".into(),
        });
    }
    let wire = crate::hosted_http::wire::encode_request(&request)
        .map_err(|failure| format!("encode exact HTTP request: {failure:?}"))?;
    stream.write_all(&wire).map_err(io_error)?;
    stream.flush().map_err(io_error)?;
    let response = crate::hosted_http::wire::read_response(stream, request.transaction_id)
        .map_err(|failure| format!("read exact HTTP response: {failure:?}"))?;
    let response = conduit_web::encode_response(&response)
        .map_err(|_| "encode bounded HTTP response".to_string())?;
    table
        .complete(lease, response.len() as u32)
        .map_err(capability_error)?;
    write(&ResponseFrame::Completed { response })
}

fn validate_bootstrap(bootstrap: &HttpBootstrap) -> Result<(), String> {
    if bootstrap.expected_authority.is_empty()
        || bootstrap.issue.scope.operation_contract_id.as_str() != HTTP_CLIENT_OPERATION
        || bootstrap.issue.scope.maximum_in_flight != 1
        || bootstrap.issue.scope.maximum_operations != 1
        || bootstrap.issue.scope.maximum_parameter_bytes
            > conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
        || bootstrap.issue.scope.maximum_result_bytes
            > conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES
    {
        return Err("HTTP Base bootstrap exceeds the exact profile".into());
    }
    Ok(())
}

fn write(frame: &ResponseFrame) -> Result<(), String> {
    crate::isolated_base::write_frame_with_limit(&mut io::stdout(), frame, MAX_FRAME_BYTES)
        .map_err(io_error)
}

fn probe_socket() -> Result<(), String> {
    let result = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if result >= 0 {
        unsafe { libc::close(result) };
        return Err("seccomp unexpectedly permitted a raw socket".into());
    }
    write_probe("raw-socket", io::Error::last_os_error())
}

fn probe_listener() -> Result<(), String> {
    let error = TcpListener::bind("127.0.0.1:0")
        .expect_err("seccomp unexpectedly permitted listener creation");
    write_probe("listener", error)
}

fn probe_process() -> Result<(), String> {
    let error = Command::new("/bin/true")
        .status()
        .expect_err("seccomp unexpectedly permitted process execution");
    write_probe("process-spawn", error)
}

fn write_probe(probe: &str, error: io::Error) -> Result<(), String> {
    write(&ResponseFrame::ProbeDenied {
        probe: probe.into(),
        os_error: error.raw_os_error().ok_or("denial had no OS error")?,
    })
}

fn apply_empty_landlock() -> Result<(), String> {
    const CREATE_RULESET_VERSION: u32 = 1;
    const HANDLED_FS_V1: u64 = (1 << 13) - 1;
    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
    }
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0,
            CREATE_RULESET_VERSION,
        )
    };
    if abi < 1 {
        return Err("isolated HTTP Base requires Landlock ABI 1".into());
    }
    let attr = RulesetAttr {
        handled_access_fs: HANDLED_FS_V1,
    };
    let ruleset = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr,
            std::mem::size_of::<RulesetAttr>(),
            0,
        ) as i32
    };
    if ruleset < 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0) } != 0
    {
        if ruleset >= 0 {
            unsafe { libc::close(ruleset) };
        }
        return Err(io_error(io::Error::last_os_error()));
    }
    unsafe { libc::close(ruleset) };
    Ok(())
}

fn descriptor_ceiling() -> Result<u16, String> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return Err(io_error(io::Error::last_os_error()));
    }
    u16::try_from(limit.rlim_cur).map_err(|_| "descriptor ceiling is not finite".into())
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

fn capability_error(error: conduit_core::BaseCapabilityRefusal) -> String {
    format!("capability:{error:?}")
}
