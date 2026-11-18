//! Initial authenticated Face and exact sealed Mask preparation.
use super::*;

impl FaceArrival {
    // Retain the route-only entrance for existing callers and CPU fixtures.
    #[allow(dead_code)]
    pub(super) fn prepare(
        host_id: HostId,
        boot_id: BootId,
        generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        provider: &NativeSurfaceProvider,
        owner_route: Option<&RemoteOwnerMaskRouteSeal>,
    ) -> Result<Self, &'static str> {
        Self::prepare_with_face(
            host_id,
            boot_id,
            generation,
            build_id,
            display_base_id,
            provider,
            owner_route,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::product_front_door) fn prepare_with_face(
        host_id: HostId,
        boot_id: BootId,
        generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        provider: &NativeSurfaceProvider,
        owner_route: Option<&RemoteOwnerMaskRouteSeal>,
        owner_face: Option<&Face>,
    ) -> Result<Self, &'static str> {
        let mask = match (owner_route, owner_face) {
            (Some(route), Some(face)) => NativeFaceMask::prepare_with_owner_face(
                host_id.clone(),
                boot_id.clone(),
                generation,
                build_id,
                display_base_id.clone(),
                "conduitos/creche/face",
                provider,
                route,
                face,
            ),
            (Some(route), None) => {
                if matches!(
                    route.planned_mask.mask.plot_name.as_str(),
                    "ambient-clock-tile" | "ambient-clock-mask"
                ) {
                    return Err("native-face-mask-face-refused");
                }
                NativeFaceMask::prepare_with_owner_route(
                    host_id.clone(),
                    boot_id.clone(),
                    generation,
                    build_id,
                    display_base_id.clone(),
                    "conduitos/creche/face",
                    provider,
                    Some(route),
                )
            }
            (None, _) => NativeFaceMask::prepare_collection_creche(
                host_id.clone(),
                boot_id.clone(),
                generation,
                build_id,
                display_base_id.clone(),
                "conduitos/creche/face",
                provider,
                conduit_face::CrecheComponentRecipe::ResponsiveColumns,
                conduit_face::ComponentVisualSelection::default(),
                conduit_face::VisualTypography::Editorial,
                false,
            ),
        }
        .map_err(|error| error.as_str())?;
        let basis = mask.birth_basis("conduitos/creche/current");
        Ok(Self {
            mask,
            basis,
            next_observation: 1,
            next_play: 1,
            next_interaction: 1,
            owner_face: None,
            preparation: NativeMaskPreparation {
                host_id,
                boot_id,
                generation,
                build_id: build_id.into(),
                display_base_id,
                provider: provider.clone(),
                minimum_text_scale_percent: 100,
                forced_colors: false,
                selected: None,
                owner_host: owner_route.map(|route| route.owner_host.clone()),
                route_plan_id: owner_route.map(|route| route.route_plan_id.clone()),
            },
        })
    }
}
