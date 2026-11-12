//! Bind the existing remote kernel to exact, already-retained Body startup.
//! This witness grants neither resources, sound authority, nor transport rights.
use super::InstalledRemoteFragment;
use crate::body_execution::CommittedBodyStartup;
use conduit_body::{PreparedBodyFragment, StartupScope};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StartupBindingState {
    Unbound,
    Bound,
    Cancelled,
}

impl InstalledRemoteFragment {
    /// Root must supply the original prepared containment and acknowledged
    /// biography. Public fragment IDs or a transport acknowledgment are not
    /// substitutes. Calling this method itself performs no kernel step/effect.
    pub fn bind_committed_body_startup(
        &mut self,
        scope: PreparedBodyFragment<'_>,
        committed: &CommittedBodyStartup,
    ) -> Result<(), String> {
        if self.startup_state != StartupBindingState::Unbound {
            return Err("remote startup is absent, bound or retired".into());
        }
        let original = scope.fragment();
        if original.fragment_id != self.lowered.identity.fragment_id
            || original.plan_id != self.lowered.identity.plan_id
            || original.placements != self.placements
        {
            return Err("retained Body startup does not contain this exact remote fragment".into());
        }
        committed.for_play(scope.plan(), scope.play())?;
        // The kernel is still unsteppable until every original source is bound.
        // A failed binding cannot make partial source state executable.
        for driver in self.scheduler.drivers() {
            if let super::super::InstalledBack::StartupSource(source) = driver {
                source.gate.bind(committed.eligible(StartupScope::Wake))?;
            }
        }
        self.startup_state = StartupBindingState::Bound;
        Ok(())
    }
}

pub(super) struct RemoteCue {
    pub(super) node: conduit_kernel::NodeId,
    output: crate::hosted_audio::startup_chime::PreparedChimeOutput,
    pub(super) pending: Option<conduit_kernel::scheduler::HostCallRequest>,
    receipt: crate::body_execution::StartupChimeCallReceipt,
}

impl InstalledRemoteFragment {
    pub(crate) fn prepare_startup_outputs(
        &mut self,
        selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<(), String> {
        if !self.startup_outputs.is_empty() {
            return Err("remote startup outputs already prepared".into());
        }
        let count = self
            .placements
            .iter()
            .filter(|p| {
                p.implementation_id.as_str() == conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
            })
            .count();
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(count)
            .map_err(|_| "remote cue capacity unavailable")?;
        for placement in &self.placements {
            if placement.implementation_id.as_str()
                != conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
            {
                continue;
            }
            super::super::startup_chime_back::validate(placement)?;
            let node = self
                .lowered
                .identity
                .node_for_placement(&placement.placement_id)
                .ok_or("remote cue node missing")?;
            outputs.push(RemoteCue {
                node,
                output: crate::hosted_audio::startup_chime::PreparedChimeOutput::prepare(
                    placement, selected,
                )?,
                pending: None,
                receipt: crate::body_execution::StartupChimeCallReceipt {
                    node,
                    request: None,
                    disposition: None,
                    failure: None,
                    kernel_accepted: false,
                },
            });
        }
        self.startup_outputs = outputs;
        Ok(())
    }

    /// Borrow bounded evidence; a transport receipt is not a cue completion.
    pub fn startup_cue_receipts(
        &self,
    ) -> impl Iterator<Item = &crate::body_execution::StartupChimeCallReceipt> {
        self.startup_outputs.iter().map(|cue| &cue.receipt)
    }

    pub(crate) fn complete_startup_host_call(
        &mut self,
        request: conduit_kernel::scheduler::HostCallRequest,
        cancelled: impl Fn() -> bool,
    ) -> Result<bool, String> {
        use conduit_kernel::HostCallDisposition;
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|op| op.node == request.node && op.call == request.call)
            .ok_or("remote request has no exact operation")?;
        if operation.contract_id.as_str() != conduit_std_offers::STARTUP_CHIME_OPERATION {
            return Ok(false);
        }
        if self.startup_state != StartupBindingState::Bound {
            return Err("remote startup is not started or is retired".into());
        }
        let cue = self
            .startup_outputs
            .iter_mut()
            .find(|cue| cue.node == request.node)
            .ok_or("remote cue has no prepared provider")?;
        if cue.pending != Some(request)
            || cue
                .receipt
                .request
                .is_some_and(|prior| prior.0 >= request.request.0)
        {
            return Err("remote cue request is not the original pending attempt".into());
        }
        let super::super::InstalledBack::StartupChime(back) =
            &self.scheduler.drivers()[usize::from(request.node.0)]
        else {
            return Err("remote cue Back changed".into());
        };
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("remote cue input: {e:?}"))?;
        // Retire the attempt before touching hardware. Even a failed kernel
        // completion cannot authorize another physical attempt from this copy.
        cue.pending = None;
        let outcome = cue.output.execute_with_cancel(input, back.pcm(), cancelled);
        cue.receipt.request = Some(request.request);
        cue.receipt.disposition = Some(outcome.disposition);
        cue.receipt.failure = outcome.failure;
        cue.receipt.kernel_accepted = false;
        if outcome.disposition == HostCallDisposition::Cancelled {
            self.cancel()?;
            return Ok(true);
        }
        self.scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|e| format!("remote cue completion: {e:?}"))?;
        cue.receipt.kernel_accepted = true;
        Ok(true)
    }
}
