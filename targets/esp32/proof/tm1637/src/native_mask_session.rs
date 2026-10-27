//! Retained ordinary Mask tables and private physical custody. No issuer or
//! Body start can be reconstructed from this preparation.
use crate::{electrical::DisplayPins, native_face::ReadLease};
use crate::{native_authority::MaskLeaseIssuer, native_mask_evidence::Consumer};
use alloc::boxed::Box;
#[path = "native_mask_terminal.rs"]
mod terminal;
use conduit_core as c;
use conduit_environment_runtime::{mask_kernel as m, mask_renderer as r};
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::prepared_session as ps};
pub type Values = &'static mut k::FixedValueStore<2, 8192>;
pub type FixedKernel = m::PreparedMaskKernel<Values>;
pub type Log = k::FixedSignLog<64, 12>;
pub type Session = ps::PreparedKernelSession<m::MaskBack, Values, Log, 3, 6, 2, 6, 6, 6, 3, 3>;
pub type NativeReady = r::ReadyRenderer<DisplayPins<'static>>;
pub type NativeCall = r::RendererCall<DisplayPins<'static>>;
pub struct FaceScope {
    pub producer: c::PlotIdentity,
    pub active: c::ActivePlayIdentity,
    pub wake: conduit_body::WakeId,
}
pub struct Preparation {
    pub context: f::PreparedMaskShowContext,
    pub kernel: FixedKernel,
    pub invocation: r::PreparedRendererInvocation,
    pub routes: f::AdmittedMaskPlotRoutes,
    pub wardrobe: f::MaskWardrobeControl,
    pub producer: FaceScope,
    pub reader: ReadLease,
}
pub struct NativeMaskSession {
    pub(super) ready: Option<NativeReady>,
    pub(super) call: Option<NativeCall>,
    pub(super) invocation: r::PreparedRendererInvocation,
    pub(super) context: Box<f::PreparedMaskShowContext>,
    pub(super) session: Session,
    pub(super) face: conduit_plan_lowering::lowering::LoweredForePort,
    pub(super) show: conduit_plan_lowering::lowering::LoweredForePort,
    pub(super) routes: f::AdmittedMaskPlotRoutes,
    pub(super) wardrobe: f::MaskWardrobeControl,
    pub(super) producer: FaceScope,
    pub(super) reader: ReadLease,
    pub(super) source_epoch: Option<(u64, u64)>,
    pub(super) index: u64,
    pub(super) pending: Option<(c::ActivePlayIdentity, k::ValueRef)>,
    pub(super) faulted: bool,
    pub(super) cleanup_refusal: Option<&'static str>,
}
impl NativeMaskSession {
    pub fn prepare(
        owner: &mut Option<NativeReady>,
        input: Preparation,
    ) -> Result<Box<Self>, &'static str> {
        let selected = input
            .wardrobe
            .selected
            .as_ref()
            .ok_or("mask-no-worn-route")?;
        if owner.is_none()
            || input.routes.body_id() != &input.wardrobe.scoped_wardrobe.body_id
            || input.wardrobe.active_plan_id != *input.routes.plan_id()
            || input.wardrobe.scoped_wardrobe.wake_id.as_ref() != Some(&input.producer.wake)
            || !input.routes.routes().iter().any(|route| {
                route.currently_available
                    && route.route_id == selected.route_id
                    && route.mask_plot == selected.mask_plot
                    && route.plan_id == selected.plan_id
            })
        {
            return Err("mask-current-route");
        }
        let m::PreparedMaskKernel {
            kernel,
            face,
            show,
            interaction,
        } = input.kernel;
        let boundary =
            |fore: &conduit_plan_lowering::lowering::LoweredForePort| ps::OwnedValueBoundary {
                endpoint: fore.endpoint,
                cord: fore.cord,
            };
        let session = Session::new_with_owned_optional_value_outputs(
            kernel,
            &[boundary(&face), boundary(&show), boundary(&interaction)],
            &[boundary(&interaction)],
            12,
        )
        .map_err(|_| "mask-session")?;
        Ok(Box::new(Self {
            ready: owner.take(),
            call: None,
            invocation: input.invocation,
            context: Box::new(input.context),
            session,
            face,
            show,
            routes: input.routes,
            wardrobe: input.wardrobe,
            producer: input.producer,
            reader: input.reader,
            source_epoch: None,
            index: 0,
            pending: None,
            faulted: false,
            cleanup_refusal: None,
        }))
    }
    pub fn cleanup_refusal(&self) -> Option<&'static str> {
        self.cleanup_refusal
    }
    pub fn completed(&self) -> u64 {
        self.index
    }
    pub fn pending_evidence(&self) -> bool {
        self.pending.is_some()
    }
    pub fn context_bytes(&self) -> usize {
        self.context.plan_wire().len()
    }
    pub(super) fn fresh_log() -> Result<Log, &'static str> {
        Log::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).ok_or("mask-sign-budget")?,
        )
        .map_err(|_| "mask-sign-budget")
    }
}

pub type RetiredKernel = k::scheduler::RetiredExecution<m::MaskBack, Values, Log, 3>;
pub(crate) type CompletionRefusal = (
    Box<NativeMaskSession>,
    Box<MaskLeaseIssuer>,
    Box<Consumer>,
    &'static str,
);
/// Opaque complete return: only actual normal session drain can construct it.
pub struct RetiredMask {
    parts: RetiredMaskParts,
}
pub struct RetiredMaskParts {
    pub renderer: NativeReady,
    pub kernel: RetiredKernel,
    pub leases: Box<MaskLeaseIssuer>,
    pub evidence: Box<Consumer>,
    pub context: Box<f::PreparedMaskShowContext>,
    pub invocation: r::PreparedRendererInvocation,
    pub routes: f::AdmittedMaskPlotRoutes,
    pub wardrobe: f::MaskWardrobeControl,
    pub producer: FaceScope,
    pub reader: ReadLease,
}
impl RetiredMask {
    pub fn renderer_identity(&self) -> &r::RendererIdentity {
        self.parts.renderer.identity()
    }
    pub fn into_parts(self) -> RetiredMaskParts {
        self.parts
    }
    pub(crate) fn matches_current_body(
        &self,
        body: &conduit_body::SealedBodyLifecycleSession,
        initial: &c::ActivePlayIdentity,
    ) -> bool {
        let parts = &self.parts;
        parts
            .leases
            .matches_current_body(&parts.renderer, body, initial)
            && body
                .realization()
                .is_some_and(|current| current.wake.wake_id == parts.producer.wake)
            && parts.context.plan_id() == parts.leases.plan_id()
            && parts.producer.active.host_id == parts.renderer.identity().host_id
            && parts.producer.active.boot_id == parts.renderer.identity().boot_id
            && !parts.kernel.cancelled
    }
}
impl NativeMaskSession {
    pub(crate) fn fresh_for_task(&self) -> bool {
        self.index == 0
            && self.pending.is_none()
            && self.call.is_none()
            && self.ready.is_some()
            && !self.faulted
            && self.cleanup_refusal.is_none()
            && self.session.kernel().decisions() == 0
    }
    pub(crate) fn retirement_ready(
        &self,
        leases: &MaskLeaseIssuer,
        evidence: &Consumer,
    ) -> Result<(), &'static str> {
        let ready = self.ready.as_ref().ok_or("mask-terminal-renderer")?;
        let facts = leases.retirement_facts(ready)?;
        let (accepted, accepted_last) = evidence.completion();
        let completion = terminal::Completion {
            completed: self.index,
            issued: facts.issued,
            accepted,
            first_sequence: facts.first,
            end_sequence: facts.end,
            next_sequence: facts.next,
            accepted_last,
            revoked: facts.revoked,
            lease_pending: facts.pending,
            first_lease_retained: facts.first_retained,
            closed_pending: self.pending.is_some(),
            call_retained: self.call.is_some(),
            renderer_retained: self.ready.is_some(),
            faulted: self.faulted,
            cleanup_refused: self.cleanup_refusal.is_some(),
            normally_drained: self.session.retirement_ready(),
        };
        if !completion.ready() {
            return Err("mask-terminal-incomplete");
        }
        if self.context.plan_id() != leases.plan_id() {
            return Err("mask-terminal-plan");
        }
        Ok(())
    }
    pub(crate) fn retire_completed(
        self: Box<Self>,
        leases: Box<MaskLeaseIssuer>,
        evidence: Box<Consumer>,
    ) -> Result<RetiredMask, CompletionRefusal> {
        if let Err(reason) = self.retirement_ready(&leases, &evidence) {
            return Err((self, leases, evidence, reason));
        }
        let Self {
            ready,
            session,
            context,
            invocation,
            routes,
            wardrobe,
            producer,
            reader,
            ..
        } = *self;
        let kernel = session
            .try_retire()
            .unwrap_or_else(|_| unreachable!("exclusive normal-drain preflight"));
        Ok(RetiredMask {
            parts: RetiredMaskParts {
                renderer: ready.expect("preflighted renderer"),
                kernel,
                leases,
                evidence,
                context,
                invocation,
                routes,
                wardrobe,
                producer,
                reader,
            },
        })
    }
}
