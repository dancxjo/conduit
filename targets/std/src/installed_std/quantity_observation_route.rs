//! Concrete quantity projection over the shared original IMAGE contribution route.
use super::prepared_face_route::{PreparedFaceProjector, PreparedFaceRoute};
use conduit_face::{FaceContributionBasis, FaceFragment, PreparedQuantityObservationSnapshotFace};

pub(super) struct QuantityObservationProjector(PreparedQuantityObservationSnapshotFace);
pub(super) type QuantityObservationRoute = PreparedFaceRoute<QuantityObservationProjector>;
impl QuantityObservationProjector {
    pub(super) fn prepare() -> Result<Self, String> {
        PreparedQuantityObservationSnapshotFace::prepare()
            .map(Self)
            .map_err(|why| format!("quantity observation Face preparation: {why:?}"))
    }
}
impl PreparedFaceProjector for QuantityObservationProjector {
    const NAME: &'static str = "Quantity observation";
    fn project(
        &self,
        basis: FaceContributionBasis,
        bytes: &[u8],
        label: &str,
    ) -> Result<FaceFragment, String> {
        self.0
            .canonical_observation(basis, bytes, label)
            .map_err(|why| format!("quantity observation semantic projection: {why:?}"))
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.0.retained_backing_bytes()
    }
    fn projection_workspace_bytes(&self) -> Option<usize> {
        self.0.projection_workspace_bytes()
    }
    fn fragment_backing_bound(&self) -> usize {
        conduit_face::quantity_observation_snapshot_fragment_backing_bound()
    }
    fn fragment_backing_bytes(&self, fragment: &FaceFragment) -> Option<usize> {
        conduit_face::inspection_fragment_backing_bytes(fragment)
    }
}
