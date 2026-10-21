//! Host preparation of the exact checked Mask and its compositor custody.
use super::*;

enum Selection<'a> {
    Ordinary,
    Owner(&'a RemoteOwnerMaskRouteSeal),
    Creche(
        conduit_face::CrecheComponentRecipe,
        conduit_face::ComponentVisualSelection,
    ),
}

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
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            route.map_or(Selection::Ordinary, Selection::Owner),
        )
    }

    /// The local zero-Body encounter selects an authored Crèche Mask root.
    /// Its geometry is checked through the same compiler, planner and resource
    /// admission as every other configured native Mask.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_creche(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        recipe: conduit_face::CrecheComponentRecipe,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_creche_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            recipe,
            conduit_face::ComponentVisualSelection::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_creche_selected(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        recipe: conduit_face::CrecheComponentRecipe,
        selection: conduit_face::ComponentVisualSelection,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            Selection::Creche(recipe, selection),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_selected(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        selection: Selection<'_>,
    ) -> Result<Self, NativeFaceMaskError> {
        let producer = NativeFaceSnapshotProducer::prepare(
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            build_id,
        )
        .map_err(NativeFaceMaskError::Producer)?;
        let mut component_configuration = None;
        let mask = match selection {
            Selection::Owner(route) => {
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
                for density in [
                    conduit_face::VisualDensity::Compact,
                    conduit_face::VisualDensity::Spacious,
                ] {
                    for palette in [
                        conduit_face::VisualPalette::Phosphor,
                        conduit_face::VisualPalette::Editorial,
                    ] {
                        let selection = conduit_face::ComponentVisualSelection { density, palette };
                        for recipe in [
                            conduit_face::TodoComponentRecipe::Flat,
                            conduit_face::TodoComponentRecipe::Grouped,
                        ] {
                            if route.planned_mask.mask.plot_name == selection.todo_root(recipe) {
                                selected = Some((selection, recipe));
                            }
                        }
                    }
                }
                let local = if let Some((selection, recipe)) = selected {
                    let (stage, configuration) =
                        conduit_conduitos_mask_offer::prepare_selected_component_stage_from_offer(
                            &offer, selection, recipe,
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
            }
            Selection::Creche(recipe, selection) => {
                let offer = crate::product_bases::native_mask_host_advertisement(
                    &host_id,
                    &boot_id,
                    offer_generation.0,
                    provider,
                );
                let (stage, configuration) =
                    conduit_conduitos_mask_offer::prepare_selected_creche_stage_from_offer(
                        &offer, recipe, selection,
                    )
                    .map_err(|_| NativeFaceMaskError::Plan)?;
                component_configuration = Some(configuration);
                stage
            }
            Selection::Ordinary => prepare_stage(
                Adapter::Native,
                &host_id,
                &boot_id,
                1,
                Some(&provider.entry),
            )
            .map_err(|_| NativeFaceMaskError::Plan)?,
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
            encounter_pending: false,
        })
    }
}
