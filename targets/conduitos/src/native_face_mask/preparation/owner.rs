//! Reconstruct the exact checked owner Mask; no Face or Plan mutation.
use super::*;
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, native_layout_host_advertisement, prepare_layout_component_document,
};
use conduit_core::HostAdvertisement;
use conduit_face::ConfiguredMaskConfiguration;

type PreparedOwner = (MaskStage, Option<ConfiguredMaskConfiguration>);

pub(super) fn prepare(
    host_id: &HostId,
    boot_id: &BootId,
    generation: OfferGeneration,
    provider: &NativeSurfaceProvider,
    route: &RemoteOwnerMaskRouteSeal,
    face: Option<&Face>,
) -> Result<PreparedOwner, NativeFaceMaskError> {
    if let Some(face) = face {
        face.validate().map_err(|_| NativeFaceMaskError::Face)?;
        if face.identity != route.face_id
            || face.revision != route.face_revision
            || face.basis != route.face_basis
            || face.basis.body_id.as_ref() != Some(&route.body_id)
        {
            return Err(NativeFaceMaskError::Face);
        }
    }
    let root = &route.planned_mask.mask.plot_name;
    let ambient = matches!(root.as_str(), "ambient-clock-tile" | "ambient-clock-mask");
    let offer = if ambient {
        native_layout_host_advertisement(
            host_id,
            boot_id,
            generation.0,
            &provider.entry,
            NativeMaskStorageProfile::Standard,
        )
    } else {
        crate::product_bases::native_mask_host_advertisement(
            host_id,
            boot_id,
            generation.0,
            provider,
        )
    };
    route
        .validate_mask_host_offer(&offer)
        .map_err(|_| NativeFaceMaskError::Plan)?;
    let (stage, configuration) = if ambient {
        let face = face.ok_or(NativeFaceMaskError::Face)?;
        let (stage, configuration) = checked_ambient(&offer, root, face)?;
        (stage, Some(configuration))
    } else if let Some((selection, recipe)) =
        conduit_face::ComponentVisualSelection::from_todo_root(root)
    {
        let (stage, configuration) =
            conduit_conduitos_mask_offer::prepare_selected_component_stage_from_offer(
                &offer, selection, recipe,
            )
            .map_err(|_| NativeFaceMaskError::Plan)?;
        (stage, Some(configuration))
    } else {
        let stage = conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            &offer,
        )
        .map_err(|_| NativeFaceMaskError::Plan)?;
        (stage, None)
    };
    exact_plan(&route.planned_mask, &stage)?;
    Ok((
        MaskStage {
            planned_mask: route.planned_mask.clone(),
        },
        configuration,
    ))
}

fn checked_ambient(
    offer: &HostAdvertisement,
    root: &str,
    face: &Face,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), NativeFaceMaskError> {
    if !matches!(root, "ambient-clock-tile" | "ambient-clock-mask") {
        return Err(NativeFaceMaskError::Plan);
    }
    let source = conduit_face::ambient_clock_mask_source_for_face(face)
        .map_err(|_| NativeFaceMaskError::Face)?;
    prepare_layout_component_document(offer, &source, root, NativeMaskStorageProfile::Standard)
        .map_err(|_| NativeFaceMaskError::Plan)
}

fn exact_plan(
    planned: &conduit_face::PlannedMaskPlot,
    local: &MaskStage,
) -> Result<(), NativeFaceMaskError> {
    if *planned != local.planned_mask {
        return Err(NativeFaceMaskError::Plan);
    }
    Ok(())
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
