//! Execute an owner-issued terminal Mask on this distinct presentation Host.
//! Membership and live carrier authority remain the caller's responsibility.
use super::*;
use conduit_face::{FaceBasis, FaceContentId, RemoteOwnerMaskRouteSeal};

pub(super) struct RemoteFaceBasis {
    identity: FaceContentId,
    revision: u64,
    basis: FaceBasis,
}
impl RemoteFaceBasis {
    pub(super) fn validate(&self, face: &Face) -> Result<(), TerminalError> {
        if face.identity != self.identity
            || face.revision != self.revision
            || face.basis != self.basis
        {
            return Err(error(
                "remote terminal Face differs from its owner-issued route",
            ));
        }
        Ok(())
    }
}

impl HostedTerminalMaskExecution {
    /// Bind an exact owner-issued Plan, including its directional Fore Lines.
    /// Only the ordinary attached terminal source and selected Backs are
    /// executable here. This does not admit a Part, discover a Line, or grant
    /// access to a remote Body: the authenticated carrier must do that first.
    pub fn new_remote_attached(
        host: &HostAdvertisement,
        route: &RemoteOwnerMaskRouteSeal,
    ) -> Result<Self, TerminalError> {
        route.validate_mask_host_offer(host).map_err(debug_error)?;
        let expected = planning::plan_attached(host)?;
        let [actual] = route.planned_mask.plan.fragments.as_slice() else {
            return Err(error("remote terminal Mask requires one fragment"));
        };
        let [local] = expected.plan.fragments.as_slice() else {
            return Err(error("ordinary terminal Mask requires one fragment"));
        };
        if route.planned_mask.mask != expected.mask
            || actual.placements != local.placements
            || actual.connections != local.connections
            || route.interaction_line.is_none()
        {
            return Err(error("remote terminal route differs from the attached Mask or lacks its typed return Line"));
        }
        let mut execution = Self::with_plan(host, route.planned_mask.clone(), false)?;
        execution.remote_face = Some(RemoteFaceBasis {
            identity: route.face_id.clone(),
            revision: route.face_revision,
            basis: route.face_basis.clone(),
        });
        Ok(execution)
    }
}
