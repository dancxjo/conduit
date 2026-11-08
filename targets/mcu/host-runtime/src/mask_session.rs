//! Continuing execution of the admitted Value-input Mask through the ordinary
//! prepared session. Local adapters are exact Fore owners, never network Lines.
use crate::execution_mask::{self, FrameTicket, OutputFrame, RendererClock, RendererService};
extern crate alloc;
use alloc::boxed::Box;
use conduit_core::SignId;
use conduit_kernel::{
    self as k,
    scheduler::{
        prepared_session::{
            DrainedInvocation, OptionalOutputStorage, OwnedValueBoundary, PreparedKernelSession,
            SessionRefusal,
        },
        HostCallRequest, RemoteIngressOutcome, SchedulerError, SchedulerStatus,
    },
};
use conduit_seven_segment_mask::native_kernel::{Kernel, MaskBack};

type Session = PreparedKernelSession<
    MaskBack,
    k::HostedValueStore,
    k::FixedSignLog<64>,
    3,
    6,
    2,
    6,
    6,
    6,
    3,
    3,
>;
#[derive(Clone, Copy)]
pub struct Boundaries {
    pub face: OwnedValueBoundary,
    pub show: OwnedValueBoundary,
    pub interaction: OwnedValueBoundary,
}
#[derive(Debug)]
pub enum Refusal {
    Session(SessionRefusal),
    Kernel(SchedulerError),
    Renderer(execution_mask::Refusal),
    Lifecycle,
}
impl From<SchedulerError> for Refusal {
    fn from(error: SchedulerError) -> Self {
        Self::Kernel(error)
    }
}
impl From<execution_mask::Refusal> for Refusal {
    fn from(error: execution_mask::Refusal) -> Self {
        Self::Renderer(error)
    }
}
/// This owner is constructed only after the complete workset has Started.
/// Storage for optional output tracking is allocated during original preparation.
/// Its previous invocation's mandatory evidence is returned explicitly on rearm.
pub struct MaskSession<'a, C: RendererClock> {
    session: Session,
    renderer: RendererService<'a, C>,
    boundaries: Boundaries,
    sequence: u64,
    active: bool,
    drained: bool,
    request: Option<HostCallRequest>,
    frame_pending: Option<FrameTicket>,
}
impl<'a, C: RendererClock> MaskSession<'a, C> {
    pub fn bind_started(
        kernel: Box<Kernel>,
        renderer: RendererService<'a, C>,
        boundaries: Boundaries,
        remote_sign_reservation: u16,
        optional: OptionalOutputStorage<6>,
    ) -> Result<Self, Refusal> {
        let session = Session::new_with_preallocated_optional_outputs(
            kernel,
            &[boundaries.face, boundaries.show, boundaries.interaction],
            &[boundaries.interaction],
            remote_sign_reservation,
            optional,
        )
        .map_err(|(error, _)| Refusal::Session(error))?;
        Ok(Self {
            session,
            renderer,
            boundaries,
            sequence: 0,
            active: false,
            drained: false,
            request: None,
            frame_pending: None,
        })
    }
    pub fn face_changed(&self) -> Result<bool, Refusal> {
        self.renderer.face_changed().map_err(Into::into)
    }
    pub fn last_event(&self) -> Option<k::KernelEvent> {
        self.session.kernel().signs().events().last()
    }
    pub fn drained_event_count(&self) -> Option<usize> {
        self.drained
            .then(|| self.session.kernel().signs().events().count())
    }
    /// Freeze the actual current Face before queueing this invocation's input.
    pub fn begin(&mut self, prepared_sign: SignId) -> Result<(), Refusal> {
        if self.active || self.drained {
            return Err(Refusal::Lifecycle);
        }
        let bytes = self.renderer.stage_face(prepared_sign)?;
        let b = self.boundaries.face;
        match self
            .session
            .admit_owned_input(b.endpoint, b.cord, self.sequence, bytes)?
        {
            RemoteIngressOutcome::Accepted { .. } => {}
            RemoteIngressOutcome::Full { .. } => return Err(Refusal::Lifecycle),
        }
        self.session.close_owned_input(b.endpoint, b.cord)?;
        self.active = true;
        Ok(())
    }
    /// Advance only the real Mask kernel. Show acceptance is a synchronous local
    /// custody transfer; electrical completion is handled separately below.
    pub fn step(
        &mut self,
        prepared_sign: &SignId,
        mut accept_show: impl FnMut(&[u8]) -> Result<(), Refusal>,
    ) -> Result<SchedulerStatus, Refusal> {
        if !self.active {
            return Err(Refusal::Lifecycle);
        }
        // Preserve the exact preflighted result and Sign custody while the
        // separately owned output kernel performs its bounded electrical call.
        if self.frame_pending.is_some() {
            return Ok(SchedulerStatus::Idle);
        }
        if self.request.is_none() {
            if let Some(request) = self.session.next_host_request() {
                let input = self.session.kernel().host_value(request.input.value)?;
                self.renderer.prepare_request(
                    request.node,
                    request.call,
                    request.request,
                    input,
                    prepared_sign.clone(),
                )?;
                self.request = Some(request);
            }
        }
        let b = self.boundaries.show;
        if let Some(offer) = self.session.owned_output(b.endpoint, b.cord)? {
            accept_show(self.session.kernel().host_value(offer.value)?)?;
            self.session
                .accept_owned_output(b.endpoint, b.cord, offer.sequence)?;
        }
        let status = self.session.step()?;
        if status == SchedulerStatus::Drained {
            self.drained = true;
        }
        Ok(status)
    }
    pub fn frame(&mut self) -> Result<Option<OutputFrame<'_>>, Refusal> {
        if self.frame_pending.is_some() {
            return Ok(None);
        }
        if self.request.is_some() {
            self.session
                .kernel()
                .preflight_host_result_storage(self.renderer.maximum_show_bytes())?;
        }
        let frame = self.renderer.frame()?;
        if let Some(frame) = &frame {
            self.frame_pending = Some(frame.ticket);
        }
        Ok(frame)
    }
    /// # Safety
    /// The caller must correlate this exact ticket with the actual admitted
    /// output fragment's successful electrical Host Call. Delivery ACKs and an
    /// unrelated successful write are not evidence for this transition.
    pub unsafe fn electrical_completion(
        &mut self,
        ticket: FrameTicket,
        available_sign: SignId,
    ) -> Result<(), Refusal> {
        if self.frame_pending != Some(ticket) {
            return Err(Refusal::Lifecycle);
        }
        // SAFETY: exact original electrical evidence is the caller's obligation.
        if let Some((request, bytes)) =
            unsafe { self.renderer.acknowledge_frame(ticket, available_sign) }?
        {
            let pending = self.request.take().ok_or(Refusal::Lifecycle)?;
            if pending.request != request {
                return Err(Refusal::Lifecycle);
            }
            let value = self.session.store_host_value(bytes)?;
            let output =
                k::BoundedValueRef::new(value, value.byte_len).map_err(|_| Refusal::Lifecycle)?;
            self.session.complete_host_call(
                pending.node,
                request,
                k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )?;
        }
        self.frame_pending = None;
        Ok(())
    }
    /// Borrow drained evidence for exact archive preflight without rearming.
    pub fn drained_evidence(&self) -> Result<DrainedInvocation<&k::FixedSignLog<64>>, Refusal> {
        if !self.drained || self.request.is_some() || self.frame_pending.is_some() {
            return Err(Refusal::Lifecycle);
        }
        self.session.drained_evidence().map_err(Refusal::Session)
    }
    /// Retain old Signs outside this owner before another rearm. The ordinary
    /// session verifies normal drain, output acceptance and unchanged bindings.
    pub fn rearm(
        &mut self,
        signs: k::FixedSignLog<64>,
    ) -> Result<DrainedInvocation<k::FixedSignLog<64>>, Refusal> {
        if !self.drained || self.request.is_some() || self.frame_pending.is_some() {
            return Err(Refusal::Lifecycle);
        }
        let next = self.sequence.checked_add(1).ok_or(Refusal::Lifecycle)?;
        let previous = self
            .session
            .begin_next([(); 3], signs)
            .map_err(|error| Refusal::Session(error.reason))?;
        self.sequence = next;
        self.active = false;
        self.drained = false;
        Ok(previous)
    }
    pub fn cancel(&mut self) -> Result<(), Refusal> {
        let renderer = self.renderer.revoke();
        let kernel = self.session.cancel();
        self.active = false;
        renderer?;
        kernel?;
        Ok(())
    }
}
