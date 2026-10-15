//! Explicit finite encoding profiles, independent of Host authority or Mask meaning.
use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteMaskRouteProfile {
    #[default]
    Native,
    HostedPresentation,
}
impl RemoteMaskRouteProfile {
    pub(super) fn is_native(&self) -> bool {
        *self == Self::Native
    }
    pub(super) fn maximum_bytes(self) -> usize {
        match self {
            Self::Native => MAX_REMOTE_ROUTE_SEAL_BYTES,
            Self::HostedPresentation => 64 * 1024,
        }
    }
}
impl RemoteOwnerMaskRouteSeal {
    /// Documentary encoding extent only; creates no route, Plan or authority.
    pub fn encoded_basis_bytes(&self) -> Result<usize, RemoteOwnerMaskRouteError> {
        self.encoded_basis().map(|bytes| bytes.len())
    }
    /// The caller must supply an actual hosted carrier with independently
    /// admitted input/output limits. This profile grants no effect authority.
    #[allow(clippy::too_many_arguments)]
    pub fn seal_current_hosted_presentation(
        session: &BodyLifecycleSession,
        face: &Face,
        owner_offer: &HostAdvertisement,
        mask_host_offer: &HostAdvertisement,
        planned_mask: &PlannedMaskPlot,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<Self, RemoteOwnerMaskRouteError> {
        Self::seal_current_inner(
            session,
            face,
            owner_offer,
            mask_host_offer,
            planned_mask,
            face_line,
            return_line,
            None,
            RemoteMaskRouteProfile::HostedPresentation,
        )
    }
}
