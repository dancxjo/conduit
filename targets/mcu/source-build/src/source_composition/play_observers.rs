// Exact local Fore/Sign bridge diagnostics; these never change admission.
#[derive(Debug)]
pub(super) enum BridgeRefusal {
    Inspection(conduit_mcu_host_runtime::execution_inspection::Refusal),
    AdmitFrame(k::scheduler::SchedulerError),
    SequenceExhausted,
    CompletionBinding(k::NodeId, k::HostCallId),
    CompletionWithoutFrame,
    HostCall(k::HostCallDisposition, Option<k::Failure>),
    CompletionSign(kernel_sign::Refusal),
}
pub(super) enum Delivery {
    Empty,
    Queued {
        ticket: FrameTicket,
        bytes: [u8; FRAME_BYTES],
        length: usize,
    },
    Admitted {
        ticket: FrameTicket,
    },
    Completed {
        ticket: FrameTicket,
        sign: c::SignId,
    },
    Failed,
}
pub(super) struct SampleObserver<'a> {
    pub(super) status: conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus,
    pub(super) inspection: &'a State<3>,
}
impl<'r, 'f> Observer<crate::play::SamplingKernel<'r, 'f>> for SampleObserver<'_> {
    type Error = BridgeRefusal;
    fn before_step(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'r, 'f>,
    ) -> Result<(), BridgeRefusal> {
        self.drain_status(kernel)
    }
    fn completed(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'r, 'f>,
        _: Dispatch,
    ) -> Result<(), BridgeRefusal> {
        self.drain_status(kernel)?;
        if let Some(event) = kernel.signs().events().last() {
            self.inspection
                .record(0, event)
                .map_err(BridgeRefusal::Inspection)?;
        }
        Ok(())
    }
}
impl SampleObserver<'_> {
    fn drain_status(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'_, '_>,
    ) -> Result<(), BridgeRefusal> {
        use conduit_kernel::ValueStorage as _;
        let Some(pin) = self.status.boundary() else {
            return Ok(());
        };
        let endpoint = k::RemoteEndpointId(pin.endpoint());
        let cord = k::CordId(pin.cord());
        let Some(offer) = kernel
            .remote_egress_offer(endpoint, cord)
            .map_err(BridgeRefusal::AdmitFrame)?
        else {
            return Ok(());
        };
        let bytes = kernel.values().get(offer.value).map_err(|_| {
            BridgeRefusal::Inspection(
                conduit_mcu_host_runtime::execution_inspection::Refusal::AppendStatus,
            )
        })?;
        self.status
            .stage(offer.sequence, bytes)
            .map_err(BridgeRefusal::Inspection)?;
        kernel
            .remote_egress_accept(endpoint, cord, offer.sequence)
            .map_err(BridgeRefusal::AdmitFrame)?;
        kernel
            .remote_egress_delivered(endpoint, cord, offer.sequence)
            .map_err(BridgeRefusal::AdmitFrame)?;
        self.status
            .delivered(self.inspection, offer.sequence)
            .map_err(BridgeRefusal::Inspection)
    }
}
pub(super) struct OutputObserver<'a> {
    pub(super) delivery: &'a RefCell<Delivery>,
    pub(super) inspection: &'a State<3>,
    pub(super) scopes: &'a Scopes,
    pub(super) evidence: &'a StartedEvidence,
    pub(super) boundary: OwnedValueBoundary,
    pub(super) sequence: u64,
    pub(super) write_node: k::NodeId,
    pub(super) last_outcome: Option<k::HostCallOutcome>,
}
impl<'r, 'f> Observer<crate::play::OutputKernel<'r, 'f>> for OutputObserver<'_> {
    type Error = BridgeRefusal;
    fn before_step(
        &mut self,
        kernel: &mut crate::play::OutputKernel<'r, 'f>,
    ) -> Result<(), BridgeRefusal> {
        let mut state = self.delivery.borrow_mut();
        if let Delivery::Queued {
            ticket,
            bytes,
            length,
        } = &*state
        {
            let ticket = *ticket;
            match kernel
                .admit_remote_input(
                    self.boundary.endpoint,
                    self.boundary.cord,
                    self.sequence,
                    &bytes[..*length],
                )
                .map_err(BridgeRefusal::AdmitFrame)?
            {
                RemoteIngressOutcome::Accepted { .. } => {
                    self.sequence = self
                        .sequence
                        .checked_add(1)
                        .ok_or(BridgeRefusal::SequenceExhausted)?;
                    *state = Delivery::Admitted { ticket };
                }
                RemoteIngressOutcome::Full { .. } => {}
            }
        }
        Ok(())
    }
    fn completed(
        &mut self,
        kernel: &mut crate::play::OutputKernel<'r, 'f>,
        dispatch: Dispatch,
    ) -> Result<(), BridgeRefusal> {
        let Dispatch::Completed { request, outcome } = dispatch else {
            return Ok(());
        };
        self.last_outcome = Some(outcome);
        if let Some(event) = kernel.signs().events().last() {
            self.inspection
                .record(2, event)
                .map_err(BridgeRefusal::Inspection)?;
        }
        if request.node != self.write_node || request.call != k::HostCallId(0) {
            return Err(BridgeRefusal::CompletionBinding(request.node, request.call));
        }
        let mut state = self.delivery.borrow_mut();
        let Delivery::Admitted { ticket } = &*state else {
            return Err(BridgeRefusal::CompletionWithoutFrame);
        };
        let ticket = *ticket;
        if outcome.disposition != k::HostCallDisposition::Completed || outcome.failure.is_some() {
            *state = Delivery::Failed;
            return Err(BridgeRefusal::HostCall(
                outcome.disposition,
                outcome.failure,
            ));
        }
        // This independently checked output Source has one input Frame and one
        // write path. Only one Frame is admitted at a time, so this completion
        // belongs to this exact ticket, never to carrier delivery or another Play.
        let sign = kernel_sign::host_call(
            self.evidence,
            self.scopes.fragment(2),
            kernel.signs(),
            request.node,
            request.request,
            kernel_sign::Phase::Completed,
            64,
        )
        .map_err(BridgeRefusal::CompletionSign)?;
        *state = Delivery::Completed { ticket, sign };
        Ok(())
    }
}

// Preserve the observed electrical disposition even if Runner retirement itself
// refuses and becomes its returned error. Compact formatting keeps both facts
// useful inside the existing 96-byte diagnostic slot.
pub(super) struct OutputExit<'a, E> {
    pub(super) outcome: Option<k::HostCallOutcome>,
    pub(super) refusal: &'a E,
}
impl<E: core::fmt::Debug> core::fmt::Debug for OutputExit<'_, E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(outcome) = self.outcome {
            write!(f, "outcome={:?}", outcome.disposition)?;
            if let Some(failure) = outcome.failure {
                write!(f, "/{:?}/{}", failure.code, failure.detail)?;
            }
            f.write_str(" ")?;
        }
        write!(f, "runner={:?}", self.refusal)
    }
}
