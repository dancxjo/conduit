//! Installed browser Back for the Source-owned canonical Patchbay actor.
//! Preparation binds the actor separately at this exact scheduler node.
use super::{factory::BrowserInstallation, patchbay_ingress, BrowserBack};
use conduit_core::PlannedGear;
use conduit_kernel::HostedValueStore;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: "browser/patchbay-accept-interaction@1",
    offer: patchbay_ingress::offer,
    prepare,
    perform: None,
};

pub(super) fn install_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "FaceInteraction",
        conduit_core::kind_id(conduit_face::FACE_INTERACTION_VALUE_KIND),
    )?;
    startup.insert(conduit_plot::KindSignature {
        kind: patchbay_application::PATCHBAY_INTERACTION_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(patchbay_application::patchbay_interaction_contract())
        .map_err(|error| format!("Patchbay interaction profile: {error:?}"))
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    // The existing ingress validator checks the complete selected finite Back;
    // only its reusable sink verb is installed in the root scheduler.
    let _validated = patchbay_ingress::prepare(placement)?;
    Ok(BrowserBack::face(
        conduit_face::MAX_FACE_INTERACTION_BYTES as u32,
        1,
    ))
}

/// Exact semantic owner detection, retaining the explicit legacy native profile.
pub(crate) fn is_owner(placement: &PlannedGear) -> Result<bool, String> {
    if placement.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND {
        return Ok(true);
    }
    Ok(
        placement.kind_id.as_str() == conduit_semantic_catalog::RETAINED_APPLICATION_KIND
            && super::application::application_id(placement)? == "patchbay",
    )
}
