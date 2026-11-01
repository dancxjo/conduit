//! Worker-local result client; actual Body admission/Line authority are external.
//! The bridge calls completion only after authenticated owner delivery. Lost
//! replies retain the existing unknown-outcome classification and are not retried.
use super::*;
use conduit_std_host::worker_text_result::WorkerTextResult;

impl DurableHostRuntime {
    // Cancellation names the actual active Play, never a caller-provided result.
    // Bound the local marker read to that identity plus one byte so an oversized
    // marker cannot grow storage or prefix-match a different identity.
    fn observe_worker_text_cancellation(&mut self) -> Result<(), String> {
        let Some(admitted) = self.remote_fragment.as_ref() else {
            return Ok(());
        };
        let Some(path) = self.cancellation_signal.as_deref() else {
            return Ok(());
        };
        let expected = admitted.identity().active_play_id.as_str().as_bytes();
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("worker-text-cancellation-unavailable:{error}")),
        };
        let mut marker = Vec::with_capacity(expected.len() + 1);
        file.take((expected.len() + 1) as u64)
            .read_to_end(&mut marker)
            .map_err(|error| format!("worker-text-cancellation-unavailable:{error}"))?;
        if marker != expected {
            return Ok(());
        }
        self.remote_fragment
            .as_mut()
            .expect("retained above")
            .cancel()?;
        self.release_remote()?;
        Err("worker-text-cancelled".into())
    }

    pub(super) fn inspect_worker_text(
        &mut self,
        identity: &ActivePlayIdentity,
        generation: u64,
    ) -> Result<Option<WorkerTextResult>, String> {
        self.observe_worker_text_cancellation()?;
        let admitted = self.remote_fragment.as_ref().ok_or("remote-play-absent")?;
        if admitted.identity() != identity
            || self.host.advertisement().offer_generation.0 != generation
        {
            return Err("worker-text-stale-play-or-offer".into());
        }
        self.pending_worker_text
            .map(|request| admitted.pending_worker_text_result(self.host.advertisement(), request))
            .transpose()
    }

    pub(super) fn complete_worker_text_to(
        &mut self,
        expected: &WorkerTextResult,
        output: &mut impl Write,
    ) -> Result<DurableRemoteExchange, String> {
        self.observe_worker_text_cancellation()?;
        let request = self
            .pending_worker_text
            .ok_or("worker-text-request-absent")?;
        let admitted = self.remote_fragment.as_mut().ok_or("remote-play-absent")?;
        let current = admitted.pending_worker_text_result(self.host.advertisement(), request)?;
        if &current != expected {
            return Err("worker-text-result-mismatch".into());
        }
        // Consume acknowledgement custody before a potentially partial effect.
        self.pending_worker_text = None;
        if let Err(error) =
            admitted.complete_worker_text_to(self.host.advertisement(), request, output)
        {
            let _ = admitted.cancel();
            self.release_remote()?;
            return Err(format!("worker-text-effect-outcome-unknown:{error}"));
        }
        let mut responses = Vec::new();
        drive_remote_fragment(
            &mut self.host,
            admitted,
            &mut self.pending_worker_text,
            &mut self.pending_native_source,
            &mut responses,
            || false,
        )?;
        Ok(DurableRemoteExchange {
            responses,
            active: admitted.sessions().iter().any(|s| s.machine().is_active()),
        })
    }
}

/// Inspect only the current locally admitted fragment. This is not a membership
/// receipt or authenticated delivery to the continuing environmental owner.
#[cfg(unix)]
#[allow(dead_code)] // Owner's admission/Line bridge supplies the live caller.
pub(crate) fn inspect(
    state_dir: &Path,
    identity: ActivePlayIdentity,
    expected_offer_generation: u64,
) -> Result<Option<WorkerTextResult>, String> {
    let response = body::call(
        state_dir,
        Request::InspectWorkerText {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            identity,
            expected_offer_generation,
        },
    )?;
    match response {
        Response::WorkerText {
            protocol: PROTOCOL,
            result,
        } => Ok(result.map(|result| *result)),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong-worker-text-response".into()),
    }
}

/// Called only after actual owner delivery through the separately admitted
/// protected Line. Local control authentication alone does not prove that fact.
#[cfg(unix)]
#[allow(dead_code)] // No fake route is installed while the bridge is absent.
pub(crate) fn complete(
    state_dir: &Path,
    result: WorkerTextResult,
) -> Result<DurableRemoteExchange, String> {
    match body::call(
        state_dir,
        Request::CompleteWorkerText {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            result: Box::new(result),
        },
    )? {
        Response::RemoteExchanged {
            protocol: PROTOCOL,
            responses,
            active,
        } => Ok(DurableRemoteExchange { responses, active }),
        Response::Refused { code, .. } => Err(code),
        _ => Err("wrong-worker-text-completion-response".into()),
    }
}

#[cfg(test)]
mod tests;
