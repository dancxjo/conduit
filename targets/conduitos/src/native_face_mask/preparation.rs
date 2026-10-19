//! Host preparation of the exact checked Mask and its compositor custody.
use super::*;

impl NativeFaceMask {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_with_owner_route(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_owner_route(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        route: Option<&RemoteOwnerMaskRouteSeal>,
    ) -> Result<Self, NativeFaceMaskError> {
        let producer = NativeFaceSnapshotProducer::prepare(
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            build_id,
        )
        .map_err(NativeFaceMaskError::Producer)?;
        let mut component_configuration = None;
        let mask = if let Some(route) = route {
            let offer = crate::product_bases::native_mask_host_advertisement(
                &host_id,
                &boot_id,
                offer_generation.0,
                provider,
            );
            route
                .validate_mask_host_offer(&offer)
                .map_err(|_| NativeFaceMaskError::Plan)?;
            let mut selected = None;
            for look in [
                conduit_face::ComponentMaskLook::Compact,
                conduit_face::ComponentMaskLook::Spacious,
            ] {
                for recipe in [
                    conduit_face::TodoComponentRecipe::Flat,
                    conduit_face::TodoComponentRecipe::Grouped,
                ] {
                    if route.planned_mask.mask.plot_name
                        == conduit_face::todo_component_mask_root(look, recipe)
                    {
                        selected = Some((look, recipe));
                    }
                }
            }
            let local = if let Some((look, recipe)) = selected {
                let (stage, configuration) =
                    conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
                        &offer, look, recipe,
                    )
                    .map_err(|_| NativeFaceMaskError::Plan)?;
                component_configuration = Some(configuration);
                stage
            } else {
                conduit_conduitos_mask_offer::prepare_stage_from_offer(
                    conduit_conduitos_mask_offer::Adapter::Native,
                    &offer,
                )
                .map_err(|_| NativeFaceMaskError::Plan)?
            };
            if route.planned_mask != local.planned_mask {
                return Err(NativeFaceMaskError::Plan);
            }
            MaskStage {
                planned_mask: route.planned_mask.clone(),
            }
        } else {
            prepare_stage(
                Adapter::Native,
                &host_id,
                &boot_id,
                1,
                Some(&provider.entry),
            )
            .map_err(|_| NativeFaceMaskError::Plan)?
        };
        let show = mask.planned_mask.show_placement();
        let compositor = NativeCompositor::admitted(
            CompositorAdmission::new(
                host_id,
                boot_id,
                offer_generation,
                show.implementation_id.clone(),
                display_base_id.clone(),
                alloc::vec![show.placement_id.clone()],
                alloc::vec![surface_id.into()],
            )
            .map_err(NativeFaceMaskError::Compositor)?,
        );
        Ok(Self {
            producer,
            mask,
            compositor,
            display_base_id,
            surface_id: surface_id.into(),
            scene: None,
            session: None,
            publication: None,
            surface_admitted: false,
            interaction_admitted: true,
            component_configuration,
            visual_expression: None,
            visual_constraints: conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
            component_fallback: None,
        })
    }

}
