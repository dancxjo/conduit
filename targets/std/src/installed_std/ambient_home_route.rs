//! Ambient composition over the shared original IMAGE/committed Face route.
use super::{
    prepared_face_route::{PreparedFaceProjector, PreparedFaceRoute},
    InstalledScheduler,
};
use crate::body_execution::face_contribution::{
    BodyFaceContributionObserver, FaceCommitObservation, FaceContributionContext,
};
use conduit_face::{FaceContributionBasis, FaceFragment};
use conduit_kernel::scheduler::HostCallRequest;
use conduit_std_offers::AmbientFaceProjection;

struct AmbientProjector {
    projection: AmbientFaceProjection,
    faces: conduit_semantic_catalog::PreparedAmbientFaces,
}
impl PreparedFaceProjector for AmbientProjector {
    const NAME: &'static str = "Home";
    fn project(
        &self,
        basis: FaceContributionBasis,
        bytes: &[u8],
        label: &str,
    ) -> Result<FaceFragment, String> {
        let mut fragment = match self.projection {
            AmbientFaceProjection::Home => self.faces.home_canonical_observation(basis, bytes),
            AmbientFaceProjection::Clock => self.faces.clock_canonical_observation(basis, bytes),
            AmbientFaceProjection::Wallpaper => {
                self.faces.wallpaper_canonical_observation(basis, bytes)
            }
        }
        .map_err(|why| format!("ambient semantic projection: {why:?}"))?;
        fragment
            .subjects
            .first_mut()
            .ok_or("Home projection has no root")?
            .name = label.to_owned();
        Ok(fragment)
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.faces.retained_backing_bytes()
    }
    fn projection_workspace_bytes(&self) -> Option<usize> {
        self.faces.projection_workspace_bytes()
    }
    fn fragment_backing_bound(&self) -> usize {
        conduit_semantic_catalog::ambient_home_fragment_backing_bound()
    }
    fn fragment_backing_bytes(&self, fragment: &FaceFragment) -> Option<usize> {
        conduit_semantic_catalog::ambient_fragment_backing_bytes(fragment)
    }
}

pub(in crate::installed_std) struct AmbientHomeRoute(PreparedFaceRoute<AmbientProjector>);
impl AmbientHomeRoute {
    pub(super) fn preparation_backing_bound(
        body: &conduit_body::BodyPlan,
        plot: &conduit_body::BodyPlotPlan,
        fragment: &conduit_core::PlanFragment,
        placement: &conduit_core::PlannedGear,
    ) -> Result<usize, String> {
        let label = super::ambient_home_factory::label(placement)?;
        PreparedFaceRoute::<AmbientProjector>::preparation_backing_bound(
            body,
            plot,
            fragment,
            placement,
            label.len(),
            conduit_semantic_catalog::ambient_home_workspace_bytes()? as usize,
            conduit_semantic_catalog::ambient_home_fragment_backing_bound(),
        )
    }
    pub(in crate::installed_std) fn prepare(
        context: FaceContributionContext<'_>,
        observation: FaceCommitObservation,
        label: &str,
        projection: AmbientFaceProjection,
    ) -> Result<Self, String> {
        PreparedFaceRoute::prepare(context, observation, label, projection.calls(), || {
            let faces = conduit_semantic_catalog::PreparedAmbientFaces::prepare()
                .map_err(|why| format!("Home model preparation: {why:?}"))?;
            Ok(AmbientProjector { projection, faces })
        })
        .map(Self)
    }
    pub(in crate::installed_std) fn attach_image_memory(
        &mut self,
        memory: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        self.0.attach_image_memory(memory);
    }
    pub(in crate::installed_std) fn additional_backing_bytes(&self) -> Result<usize, String> {
        self.0.additional_backing_bytes()
    }
    pub(in crate::installed_std) fn escaped_backing_bytes(&self) -> Result<usize, String> {
        self.0.escaped_backing_bytes()
    }
    pub(in crate::installed_std) fn dispatch(
        &mut self,
        kernel: &mut InstalledScheduler,
        request: HostCallRequest,
    ) -> Result<(), String> {
        self.0.dispatch(kernel, request)
    }
    pub(in crate::installed_std) fn poll(
        &mut self,
        kernel: &mut InstalledScheduler,
        observer: &mut dyn BodyFaceContributionObserver,
    ) -> Result<bool, String> {
        self.0.poll(kernel, observer)
    }
    pub(in crate::installed_std) fn handles(&self, request: HostCallRequest) -> bool {
        self.0.handles(request)
    }
    pub(in crate::installed_std) fn is_pending(&self) -> bool {
        self.0.is_pending()
    }
    pub(in crate::installed_std) fn is_retired(&self) -> bool {
        self.0.is_retired()
    }
}
