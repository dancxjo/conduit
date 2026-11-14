//! Process-local scope retained only after a pinned owner exchange.
//! This is not wire authority or a membership/Line grant.
use super::*;
use conduit_core::PlanId;
use conduit_face::FaceContentId;

pub(super) struct AdmittedSnapshot {
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    route: PlanId,
    mask_plan: PlanId,
    face: FaceContentId,
    revision: u64,
}
impl AdmittedSnapshot {
    pub(super) fn bind(
        snapshot: &Snapshot,
        request: &OwnerFaceSnapshotRequest,
    ) -> Result<Self, String> {
        let route = &snapshot.route;
        let face = &snapshot.face;
        face.validate()
            .map_err(|_| "owner returned an invalid Face")?;
        if route.body_id != request.body_id
            || face.basis.body_id.as_ref() != Some(&request.body_id)
            || route.mask_host.host_id != request.host_id
            || route.mask_host.boot_id != request.boot_id
            || route.face_id != face.identity
            || route.face_revision != face.revision
            || route.face_basis != face.basis
        {
            return Err("owner snapshot differs from this admitted encounter".into());
        }
        Ok(Self {
            #[cfg(all(target_os = "linux", feature = "native-display"))]
            route: route.route_plan_id.clone(),
            mask_plan: route.planned_mask.plan.plan_id.clone(),
            face: face.identity.clone(),
            revision: face.revision,
        })
    }
    pub(super) fn validate_show(&self, show: &MaskShow) -> Result<(), String> {
        if show.face_id != self.face
            || show.face_revision != self.revision
            || show.planned_mask.plan.plan_id != self.mask_plan
        {
            return Err("Show differs from the current owner snapshot".into());
        }
        Ok(())
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(super) fn validate_native(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
    ) -> Result<(), String> {
        if route.route_plan_id != self.route
            || route.planned_mask.plan.plan_id != self.mask_plan
            || face.identity != self.face
            || face.revision != self.revision
            || route.face_id != face.identity
            || route.face_revision != face.revision
            || route.face_basis != face.basis
            || route.interaction_line.is_some()
        {
            return Err("native frame differs from the authenticated owner snapshot".into());
        }
        Ok(())
    }
}
