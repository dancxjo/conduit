//! One prepared provider lease for an exact planned finite microphone source.
use super::{microphone_flow_back, microphone_host, InstalledScheduler};
use crate::hosted_microphone::{
    AlsaMicrophoneAdapter, MicrophoneFailure, MicrophoneFlow, MicrophoneFlowPoll,
    PreparedMicrophoneFlow,
};
use conduit_core::PlanFragment;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome, NodeId,
};

pub(super) struct MicrophoneFlowHost {
    node: Option<NodeId>,
    prepared: Option<PreparedMicrophoneFlow>,
    flow: Option<MicrophoneFlow>,
    pending: Option<HostCallRequest>,
    next_request: u32,
}
impl MicrophoneFlowHost {
    pub fn prepare(
        fragment: &PlanFragment,
        microphone: Option<&AlsaMicrophoneAdapter>,
    ) -> Result<Self, String> {
        let mut selected = fragment
            .placements
            .iter()
            .enumerate()
            .filter(|(_, placement)| {
                matches!(
                    placement.implementation_id.as_str(),
                    conduit_std_offers::MICROPHONE_FLOW_IMPLEMENTATION
                        | conduit_std_offers::CONVERTED_MICROPHONE_FLOW_IMPLEMENTATION
                )
            });
        let selected = match (selected.next(), selected.next()) {
            (None, None) => None,
            (Some(value), None) => Some(value),
            _ => {
                return Err("one microphone endpoint cannot prepare multiple capture leases".into())
            }
        };
        let (node, prepared) = if let Some((node, placement)) = selected {
            let adapter = microphone.ok_or("finite capture lacks an initialized microphone")?;
            if adapter.flow_offer().implementation.implementation_id != placement.implementation_id
            {
                return Err("finite capture format differs from selected provider".into());
            }
            let (millis, frames) = microphone_flow_back::configuration(placement)?;
            let prepared = adapter
                .prepare_pcm_flow_for(millis, frames)
                .map_err(|error| format!("prepare finite capture: {error}"))?;
            (
                Some(NodeId(
                    u16::try_from(node).map_err(|_| "capture node bound")?,
                )),
                Some(prepared),
            )
        } else {
            (None, None)
        };
        Ok(Self {
            node,
            prepared,
            flow: None,
            pending: None,
            next_request: 0,
        })
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn cancel(&mut self) {
        self.flow = None;
        self.prepared = None;
        self.pending = None;
    }
    pub fn accept(
        &mut self,
        request: HostCallRequest,
        scheduler: &mut InstalledScheduler,
        cancelled: bool,
    ) -> Result<(), String> {
        if self.node != Some(request.node)
            || self.pending.is_some()
            || request.request.0 != self.next_request
        {
            return Err("finite capture request does not match prepared lease".into());
        }
        let expected = if self.next_request == 0 {
            b"capture".as_slice()
        } else {
            b"next".as_slice()
        };
        let valid = scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read finite capture request: {error:?}"))?
            == expected;
        self.pending = Some(request);
        if !valid {
            return self.complete(
                scheduler,
                HostCallOutcome {
                    disposition: HostCallDisposition::Denied,
                    output: None,
                    failure: Some(Failure {
                        code: FailureCode::InvalidInput,
                        detail: 20,
                    }),
                },
            );
        }
        if let Some(prepared) = self.prepared.take() {
            match prepared.start(cancelled) {
                Ok(flow) => self.flow = Some(flow),
                Err(error) => {
                    return self.complete(scheduler, microphone_host::failure_outcome(error))
                }
            }
        }
        Ok(())
    }
    fn complete(
        &mut self,
        scheduler: &mut InstalledScheduler,
        outcome: HostCallOutcome,
    ) -> Result<(), String> {
        let request = self
            .pending
            .take()
            .ok_or("finite capture has no pending call")?;
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or("finite capture request identity exhausted")?;
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|error| format!("complete finite capture: {error:?}"))
    }
    pub fn poll(
        &mut self,
        scheduler: &mut InstalledScheduler,
        cancelled: bool,
    ) -> Result<bool, String> {
        if self.pending.is_none() {
            return Ok(false);
        }
        let Some(flow) = self.flow.as_mut() else {
            self.complete(
                scheduler,
                microphone_host::failure_outcome(MicrophoneFailure::ProviderLost),
            )?;
            return Ok(true);
        };
        let outcome = match flow.poll(cancelled) {
            Ok(MicrophoneFlowPoll::Pending) => return Ok(false),
            Ok(MicrophoneFlowPoll::Frame) => {
                let frame = flow
                    .frame()
                    .ok_or("finite capture did not retain its frame")?;
                let value = scheduler
                    .store_host_value(frame)
                    .map_err(|error| format!("store finite capture frame: {error:?}"))?;
                flow.release_frame().map_err(|error| error.to_string())?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            value,
                            conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_BLOCK_BYTES,
                        )
                        .map_err(|error| format!("bound finite capture frame: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
            Ok(MicrophoneFlowPoll::Complete) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
            Err(error) => microphone_host::failure_outcome(error),
        };
        self.complete(scheduler, outcome)?;
        Ok(true)
    }
}
