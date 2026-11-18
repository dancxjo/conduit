//! Current authenticated encounter and separately authorized placement policy.
use super::*;

/// Trusted cooperative hosted carrier adapter. Implementations must retain and
/// recheck authenticated membership, pinned peer, exact Face/route/Host binding
/// and their current finite lifetime. Received wire data alone is insufficient.
/// This is an explicit hosted TCB seam, not a hostile-code capability boundary.
pub trait NativeMaskEncounter {
    fn validate_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local_offer: &HostAdvertisement,
    ) -> Result<(), String>;

    /// Additional trusted enclosing-owner obligation, separate from route truth.
    /// Implementers accepting a tile must retain the actual allocation decision
    /// and recheck its current physical surface, Host/Boot and lifetime here.
    /// A requested rectangle is not its own placement or resource authority.
    /// Existing carriers support only the entire original physical surface.
    fn validate_presentation_container_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local_offer: &HostAdvertisement,
        physical: conduit_face::LayoutRect,
        requested: conduit_face::LayoutRect,
    ) -> Result<(), String> {
        self.validate_current(face, route, local_offer)?;
        fullscreen_only(physical, requested)
    }
}
pub(super) fn validate_encounter(
    encounter: &impl NativeMaskEncounter,
    route: &RemoteOwnerMaskRouteSeal,
    local: &HostAdvertisement,
    face: &Face,
) -> Result<(), String> {
    route
        .validate_mask_host_offer(local)
        .map_err(|error| format!("selected native local route: {error:?}"))?;
    if route.face_id != face.identity
        || route.face_revision != face.revision
        || route.face_basis != face.basis
    {
        return Err("selected native Face differs from exact owner route".into());
    }
    encounter.validate_current(face, route, local)
}

fn fullscreen_only(
    physical: conduit_face::LayoutRect,
    requested: conduit_face::LayoutRect,
) -> Result<(), String> {
    if physical != requested {
        return Err("native enclosing owner has not allocated this presentation container".into());
    }
    Ok(())
}

pub(super) fn validate_presentation_encounter(
    encounter: &impl NativeMaskEncounter,
    route: &RemoteOwnerMaskRouteSeal,
    local: &HostAdvertisement,
    face: &Face,
    physical: conduit_face::LayoutRect,
    requested: conduit_face::LayoutRect,
) -> Result<(), String> {
    validate_encounter(encounter, route, local, face)?;
    encounter.validate_presentation_container_current(face, route, local, physical, requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_owner_allows_only_the_original_fullscreen_rectangle() {
        let physical = conduit_face::LayoutRect {
            x: 0,
            y: 0,
            width: 1024,
            height: 600,
        };
        assert!(fullscreen_only(physical, physical).is_ok());
        for requested in [
            conduit_face::LayoutRect {
                x: 16,
                y: 16,
                width: 320,
                height: 320,
            },
            conduit_face::LayoutRect {
                x: 0,
                y: 0,
                width: 512,
                height: 600,
            },
        ] {
            assert!(fullscreen_only(physical, requested).is_err());
        }
    }
}
