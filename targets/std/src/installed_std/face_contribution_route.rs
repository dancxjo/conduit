//! One installed inventory of concrete projectors over the same contribution shell.
use super::{
    ambient_home_route::AmbientHomeRoute,
    back::InstalledBack,
    quantity_observation_route::{QuantityObservationProjector, QuantityObservationRoute},
    InstalledScheduler,
};
use crate::body_execution::face_contribution::{
    BodyFaceContributionObserver, FaceContributionContext,
};
use conduit_core::PlannedGear;
use conduit_kernel::scheduler::HostCallRequest;

// Keep the selected route inline in its finite inventory. Preflight charges
// size_of::<Self>() (including variant padding), and the actual IMAGE visitor
// measures that same Vec capacity. Boxing would add a separate heap owner.
#[allow(clippy::large_enum_variant)]
pub(super) enum InstalledFaceRoute {
    Ambient(AmbientHomeRoute),
    Quantity(QuantityObservationRoute),
}
macro_rules! route_method {
    ($name:ident (&self) -> $result:ty) => {
        pub(super) fn $name(&self) -> $result {
            match self {
                Self::Ambient(route) => route.$name(),
                Self::Quantity(route) => route.$name(),
            }
        }
    };
}
impl InstalledFaceRoute {
    pub(super) fn prepare(
        context: FaceContributionContext<'_>,
        placement: &PlannedGear,
        back: &InstalledBack,
    ) -> Result<Self, String> {
        if placement.implementation_id.as_str()
            == conduit_std_offers::QUANTITY_OBSERVATION_FACE_IMPLEMENTATION
        {
            let InstalledBack::QuantityObservationFace(back) = back else {
                return Err("quantity Face call has no matching installed Back".into());
            };
            return QuantityObservationRoute::prepare(
                context,
                back.observation(),
                super::quantity_observation_factory::label(placement)?,
                (
                    conduit_std_offers::QUANTITY_OBSERVATION_PROJECT_CALL,
                    conduit_std_offers::QUANTITY_OBSERVATION_CUSTODY_CALL,
                ),
                QuantityObservationProjector::prepare,
            )
            .map(Self::Quantity);
        }
        let InstalledBack::AmbientHome(back) = back else {
            return Err("Home Face project call has no installed Home Back".into());
        };
        AmbientHomeRoute::prepare(
            context,
            back.observation(),
            super::ambient_home_factory::label(placement)?,
            super::ambient_home_factory::projection(placement)?,
        )
        .map(Self::Ambient)
    }
    pub(super) fn preparation_backing_bound(
        body: &conduit_body::BodyPlan,
        plot: &conduit_body::BodyPlotPlan,
        fragment: &conduit_core::PlanFragment,
        placement: &PlannedGear,
    ) -> Result<usize, String> {
        let (bound, inline) = if placement.implementation_id.as_str()
            == conduit_std_offers::QUANTITY_OBSERVATION_FACE_IMPLEMENTATION
        {
            (
                QuantityObservationRoute::preparation_backing_bound(
                    body,
                    plot,
                    fragment,
                    placement,
                    super::quantity_observation_factory::label(placement)?.len(),
                    conduit_semantic_catalog::quantity_observation_face_workspace_bytes()? as usize,
                    conduit_face::quantity_observation_snapshot_fragment_backing_bound(),
                )?,
                core::mem::size_of::<QuantityObservationRoute>(),
            )
        } else {
            (
                AmbientHomeRoute::preparation_backing_bound(body, plot, fragment, placement)?,
                core::mem::size_of::<AmbientHomeRoute>(),
            )
        };
        bound
            .checked_add(core::mem::size_of::<Self>().saturating_sub(inline))
            .ok_or_else(|| "Face route inventory backing overflow".into())
    }
    pub(super) fn attach_image_memory(
        &mut self,
        memory: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        match self {
            Self::Ambient(route) => route.attach_image_memory(memory),
            Self::Quantity(route) => route.attach_image_memory(memory),
        }
    }
    pub(super) fn dispatch(
        &mut self,
        kernel: &mut InstalledScheduler,
        request: HostCallRequest,
    ) -> Result<(), String> {
        match self {
            Self::Ambient(route) => route.dispatch(kernel, request),
            Self::Quantity(route) => route.dispatch(kernel, request),
        }
    }
    pub(super) fn poll(
        &mut self,
        kernel: &mut InstalledScheduler,
        observer: &mut dyn BodyFaceContributionObserver,
    ) -> Result<bool, String> {
        match self {
            Self::Ambient(route) => route.poll(kernel, observer),
            Self::Quantity(route) => route.poll(kernel, observer),
        }
    }
    pub(super) fn handles(&self, request: HostCallRequest) -> bool {
        match self {
            Self::Ambient(route) => route.handles(request),
            Self::Quantity(route) => route.handles(request),
        }
    }
    route_method!(additional_backing_bytes(&self) -> Result<usize, String>);
    route_method!(escaped_backing_bytes(&self) -> Result<usize, String>);
    route_method!(is_pending(&self) -> bool);
    route_method!(is_retired(&self) -> bool);
}
