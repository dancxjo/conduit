//! Exact Browser Host installation and preparation for Wallpaper.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserBack,
};
use crate::browser_wallpaper::BrowserWallpaperObservation;
use crate::browser_wallpaper_back::BrowserWallpaperBack;
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    PlannedGear, RUNTIME_MEMORY_RESOURCE_CLASS,
};
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_semantic_catalog as catalog;

const ID: &str = "browser/ambient-wallpaper@1";
const PREPARED_BYTES: u32 = 64 * 1024;
pub(crate) const OPERATION: &str = "conduit.host/browser-wallpaper-clock@1";
pub(crate) const RESOURCE: &str = "conduit.resource/browser-wallpaper-slot@1";
pub(crate) const POOL: &str = "browser/wallpaper";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: ID,
    offer,
    prepare: missing_deck,
    perform: None,
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        catalog::resident_wallpaper_kind().expect("checked Wallpaper Kind"),
        Back {
            capability_id: CapabilityId::from(ID),
            execution_profile_id: ExecutionProfileId::from(ID),
            implementation_id: ImplementationId::from(ID),
            artifact_id: ArtifactId::from("conduit-browser-runtime/ambient-wallpaper@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(OPERATION),
                target_kind: Some(kind_id(catalog::RESIDENT_WALLPAPER_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: 8,
            }],
            resource_requirements: vec![
                resource_requirement(RESOURCE, 1),
                resource_requirement(RUNTIME_MEMORY_RESOURCE_CLASS, PREPARED_BYTES),
            ],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn missing_deck(_: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    Err("browser wallpaper requires its prepared Body Host resource".into())
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreparedWallpaper {
    placement_id: conduit_core::PlacementId,
    observation: String,
}

pub(crate) fn prepare_for_body(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
    deck: Option<&PreparedWallpaper>,
) -> Result<Option<BrowserBack>, String> {
    if placement.implementation_id.as_str() != ID {
        return Ok(None);
    }
    validate_placement(placement, &offer())?;
    let deck = deck
        .filter(|deck| deck.placement_id == placement.placement_id)
        .ok_or("wallpaper preparation differs from the selected placement")?;
    let observation = BrowserWallpaperObservation::prepare(deck.observation.as_bytes())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("prepare wallpaper request: {error:?}"))?;
    let back = BrowserWallpaperBack::prepare(observation, empty, PREPARED_BYTES as usize)?;
    Ok(Some(BrowserBack::installed_step(back)))
}
