//! Actual terminal evidence never retires the owner before member release ACK.
use super::*;
use conduit_std_host::body_execution::DelegatedBodyTerminalReceipt;

pub(super) fn finish(
    owner: &mut Owner,
    root: &Path,
    terminal: &DelegatedBodyTerminalReceipt,
    release: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    if owner.session.realization().and_then(|r| r.play.as_ref()) != Some(&terminal.global_play) {
        return Err("member terminal differs from original retained Play".into());
    }
    let receipt = serde_json::json!({"schema":"conduit.body/member-execution@1","disposition":"terminal","terminal":terminal,"member_release":"pending"});
    owner.retain_execution(root, owner.session.clone(), receipt)?;
    if terminal.cleanup_failure.is_some() {
        let mut receipt = owner
            .last_execution
            .clone()
            .ok_or("terminal receipt vanished")?;
        receipt["member_release"] = "cleanup-unknown".into();
        owner.retain_execution(root, owner.session.clone(), receipt)?;
        return Ok(false);
    }
    if release().is_err() {
        let mut receipt = owner
            .last_execution
            .clone()
            .ok_or("terminal receipt vanished")?;
        receipt["member_release"] = "unknown".into();
        owner.retain_execution(root, owner.session.clone(), receipt)?;
        return Ok(false);
    }
    let mut receipt = owner
        .last_execution
        .clone()
        .ok_or("terminal receipt vanished")?;
    receipt["member_release"] = "released".into();
    let authority = owner.host.advertisement();
    let mut next = owner.session.clone();
    next.lull(
        &authority.host_id,
        &authority.boot_id,
        Some(&terminal.global_play),
    )
    .map_err(debug)?;
    // Keep the original Playing session if the final durable Lull is ambiguous.
    // Member release is known; neither a second release nor replay is inferred.
    if let Err(error) = super::super::state::retain_session(
        root,
        &mut next,
        Some(&receipt),
        owner.admissions.as_ref(),
        None,
        None,
    ) {
        owner.last_execution = Some(receipt);
        return Err(error);
    }
    owner.session = next;
    owner.last_execution = Some(receipt);
    Ok(true)
}
#[cfg(test)]
#[path = "terminal_retention/tests.rs"]
mod tests;
