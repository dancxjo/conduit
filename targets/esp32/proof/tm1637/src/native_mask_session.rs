//! Retained ordinary Mask tables and private physical custody. No issuer or
//! Body start can be reconstructed from this preparation.
use crate::{electrical::DisplayPins, native_face::Reader};
use alloc::boxed::Box;
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
    pub reader: &'static Reader,
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
    pub(super) reader: &'static Reader,
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
