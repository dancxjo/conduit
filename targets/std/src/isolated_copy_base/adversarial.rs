use super::{io_error, CopyBootstrap, RequestFrame, ResponseFrame, PROTOCOL_VERSION};
use conduit_core::BaseOperationClaim;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdversarialCopyReport {
    pub forged_scope_refused: bool,
    pub stale_generation_refused_after_restart: bool,
    pub sibling_read_errno: i32,
    pub sibling_write_errno: i32,
    pub process_spawn_errno: i32,
}

pub fn run_adversarial_copy_proof(
    executable: &Path,
    bootstrap: CopyBootstrap,
    mut forged_claim: BaseOperationClaim,
    sibling: &Path,
) -> Result<AdversarialCopyReport, String> {
    let stale_claim = forged_claim.clone();
    let mut restart_bootstrap = bootstrap.clone();
    restart_bootstrap.issue.authority.base_provider_generation += 1;
    restart_bootstrap.issue.scope.base_provider_generation += 1;
    forged_claim.subject_kind = conduit_core::KindId::from("file/protected-sibling");
    let mut child = crate::isolated_base::spawn_provider(executable).map_err(io_error)?;
    let mut input = child.stdin.take().ok_or("copy proof stdin missing")?;
    let mut output = child.stdout.take().ok_or("copy proof stdout missing")?;
    crate::isolated_base::write_frame(
        &mut input,
        &RequestFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            bootstrap: Box::new(bootstrap),
        },
    )
    .map_err(io_error)?;
    if !matches!(
        crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?,
        ResponseFrame::Ready { .. }
    ) {
        return Err("copy proof provider was not ready".into());
    }
    crate::isolated_base::write_frame(
        &mut input,
        &RequestFrame::Step {
            claim: Box::new(forged_claim),
        },
    )
    .map_err(io_error)?;
    let forged_scope_refused = matches!(
        crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?,
        ResponseFrame::Refused { reason } if reason == "capability:WrongScope"
    );
    let mut probe = |request: RequestFrame, expected: &str| -> Result<i32, String> {
        crate::isolated_base::write_frame(&mut input, &request).map_err(io_error)?;
        match crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)? {
            ResponseFrame::ProbeDenied { probe, os_error } if probe == expected => Ok(os_error),
            _ => Err(format!("copy proof did not receive {expected} denial")),
        }
    };
    let sibling_read_errno = probe(
        RequestFrame::ProbeReadPath {
            path: sibling.into(),
        },
        "raw-sibling-read",
    )?;
    let sibling_write_errno = probe(
        RequestFrame::ProbeWritePath {
            path: sibling.into(),
        },
        "raw-sibling-write",
    )?;
    let process_spawn_errno = probe(RequestFrame::ProbeProcessSpawn, "process-spawn")?;
    crate::isolated_base::write_frame(&mut input, &RequestFrame::Cancel).map_err(io_error)?;
    let _ = crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?;
    crate::isolated_base::write_frame(&mut input, &RequestFrame::Shutdown).map_err(io_error)?;
    let _ = crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?;
    drop(input);
    if !child.wait().map_err(io_error)?.success() {
        return Err("copy proof provider failed".into());
    }
    let stale_generation_refused_after_restart =
        stale_after_restart(executable, restart_bootstrap, stale_claim)?;
    Ok(AdversarialCopyReport {
        forged_scope_refused,
        stale_generation_refused_after_restart,
        sibling_read_errno,
        sibling_write_errno,
        process_spawn_errno,
    })
}

fn stale_after_restart(
    executable: &Path,
    bootstrap: CopyBootstrap,
    stale_claim: BaseOperationClaim,
) -> Result<bool, String> {
    let mut child = crate::isolated_base::spawn_provider(executable).map_err(io_error)?;
    let mut input = child.stdin.take().ok_or("restart proof stdin missing")?;
    let mut output = child.stdout.take().ok_or("restart proof stdout missing")?;
    crate::isolated_base::write_frame(
        &mut input,
        &RequestFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            bootstrap: Box::new(bootstrap),
        },
    )
    .map_err(io_error)?;
    if !matches!(
        crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?,
        ResponseFrame::Ready { .. }
    ) {
        return Err("replacement copy provider was not ready".into());
    }
    crate::isolated_base::write_frame(
        &mut input,
        &RequestFrame::Step {
            claim: Box::new(stale_claim),
        },
    )
    .map_err(io_error)?;
    let refused = matches!(
        crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?,
        ResponseFrame::Refused { reason } if reason == "capability:WrongScope"
    );
    crate::isolated_base::write_frame(&mut input, &RequestFrame::Cancel).map_err(io_error)?;
    let _ = crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?;
    crate::isolated_base::write_frame(&mut input, &RequestFrame::Shutdown).map_err(io_error)?;
    let _ = crate::isolated_base::read_frame::<_, ResponseFrame>(&mut output).map_err(io_error)?;
    drop(input);
    if !child.wait().map_err(io_error)?.success() {
        return Err("replacement copy provider failed".into());
    }
    Ok(refused)
}
