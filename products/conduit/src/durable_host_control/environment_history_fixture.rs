//! Test-only custody handoff to the ordinary control actor. No replacement
//! Owner, Host, scheduler, membership transcript or Source observation is made.
use super::{body::HostSource, body_run::OwnedRunWorker, service, DurableHostRuntime};
use crate::durable_host::owner::{Owner, RunWorker};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Caller-supplied service inspection metadata. This fixture neither validates
/// an installed release nor derives execution or effect authority from it.
pub(crate) struct FixtureIdentity {
    pub target_id: String,
    pub image_content_digest: String,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FixtureStop {
    Requested,
    Deadline,
    CallbackFailed(String),
}
pub(crate) struct ServedOwner {
    pub owner: Owner,
    pub stop: FixtureStop,
}

/// Run the same serialized local control loop as `host service run`. A lulled
/// Owner with retained workset and `None` worker lets a fresh WASM Host join
/// before the normal control client starts the actual Source Play.
///
/// `stop` must be nonblocking (for example `Child::try_wait`). Callback completion
/// only requests Lull: the existing actor still progresses and retires the worker.
/// A deadline never becomes successful browser evidence. Neither this helper nor
/// its return value acknowledges a Show, producer, recorder or physical effect.
#[allow(clippy::too_many_arguments)]
pub(crate) fn serve_owner(
    root: &Path,
    owner: Owner,
    worker: Option<RunWorker>,
    identity: FixtureIdentity,
    maximum: Duration,
    retirement_timeout: Duration,
    mut stop: impl FnMut() -> Result<bool, String>,
) -> Result<ServedOwner, String> {
    if maximum.is_zero() || retirement_timeout.is_zero() {
        return Err(
            "Owner fixture requires finite nonzero service and retirement deadlines".into(),
        );
    }
    let deadline = Instant::now()
        .checked_add(maximum)
        .ok_or("Owner fixture service deadline overflow")?;
    if identity.target_id.is_empty() || identity.image_content_digest.is_empty() {
        return Err("Owner fixture requires explicit service inspection metadata".into());
    }
    // The ordinary secret API retains an existing valid token or creates one
    // under its normal exclusive0600 file rule. Never expose token material.
    super::ensure_secret(root)?;
    let expected_host = owner.host.advertisement().host_id.clone();
    let expected_boot = owner.host.advertisement().boot_id.clone();
    let runtime = DurableHostRuntime {
        target_id: identity.target_id,
        image_content_digest: identity.image_content_digest,
        host: HostSource::Body {
            owner: Box::new(owner),
            root: root.to_path_buf(),
            running: worker.map(OwnedRunWorker::General),
        },
        body_renewal: None,
        birth: None,
        birth_root: Some(root.to_path_buf()),
        remote_fragment: None,
        startup_member: None,
        pending_worker_text: None,
        pool_member: None,
        cancellation_signal: None,
        next_observation_sequence: 0,
        terminal_route: None,
        selected_speech_equipment: None,
        speech_worker: None,
        speech_terminal: None,
        owner_spoken_worker: None,
        owner_spoken_terminal: None,
        spoken_template_session: None,
    };
    const RETIRED: &str = "environment-owner-fixture-retired";
    let mut reason = None;
    let mut retirement_deadline = None;
    let mut retained = None;
    let result = service::serve_with_test_foreground(root, runtime, &mut |runtime| {
        if reason.is_none() {
            reason = if Instant::now() >= deadline {
                Some(FixtureStop::Deadline)
            } else {
                match stop() {
                    Ok(true) => Some(FixtureStop::Requested),
                    Err(error) => Some(FixtureStop::CallbackFailed(error)),
                    Ok(false) => None,
                }
            };
            if reason.is_some() {
                retirement_deadline = Some(
                    Instant::now()
                        .checked_add(retirement_timeout)
                        .ok_or("Owner fixture retirement deadline overflow")?,
                );
                runtime.request_owned_body_lull()?;
            }
        }
        if reason.is_none() {
            return Ok(());
        }
        if retirement_deadline.is_some_and(|until| Instant::now() >= until) {
            return Err(
                "Owner fixture failed to retire after ordinary Lull before its deadline".into(),
            );
        }
        let HostSource::Body { owner, running, .. } = &runtime.host else {
            return Err("Owner fixture lost its original Body custody".into());
        };
        if running.is_none() && owner.current_play_id().is_none() {
            let host = owner.host.advertisement();
            if host.host_id != expected_host || host.boot_id != expected_boot {
                return Err("Owner fixture retired a different Host/Boot".into());
            }
            let HostSource::Body {
                owner,
                running: None,
                ..
            } = std::mem::replace(&mut runtime.host, HostSource::Transitioning)
            else {
                return Err("Owner fixture retirement custody changed".into());
            };
            retained = Some(owner);
            return Err(RETIRED.into());
        }
        Ok(())
    });
    match (result, retained, reason) {
        (Err(error), Some(owner), Some(stop)) if error == RETIRED => Ok(ServedOwner {
            owner: *owner,
            stop,
        }),
        (Err(error), _, _) => Err(error),
        _ => Err("Owner fixture control loop ended without ordinary retirement".into()),
    }
}
