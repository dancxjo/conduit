use conduit_core::{BootId, HostId};
use patchbay_model::ZeroBodyFrontDoor;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePatchbayOpening {
    pub host_id: String,
    pub boot_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
}

/// Open Patchbay's canonical zero-Body front door and project its portable
/// Presentation. The interactive application owns window creation separately.
pub fn open_patchbay_presentation() -> Result<NativePatchbayOpening, String> {
    let session = ZeroBodyFrontDoor::with_identity(
        Arc::new(patchbay_hosted::HostedPatchbayAdapter),
        HostId::from("conduit-home/native-patchbay"),
        BootId::from("conduit-home/native-patchbay/boot-1"),
    )?;
    let host = session.advertisement();
    let projection = session.project()?;
    projection
        .presentation
        .validate()
        .map_err(|error| format!("Patchbay Presentation refused: {error:?}"))?;

    Ok(NativePatchbayOpening {
        host_id: host.host_id.as_str().to_owned(),
        boot_id: host.boot_id.as_str().to_owned(),
        presentation_id: projection.presentation.identity.as_str().to_owned(),
        presentation_revision: projection.presentation.revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patchbay_opening_is_a_real_portable_presentation() {
        let opening = open_patchbay_presentation().unwrap();
        assert!(!opening.host_id.is_empty());
        assert!(!opening.boot_id.is_empty());
        assert!(!opening.presentation_id.is_empty());
        assert!(opening.presentation_revision > 0);
    }
}
