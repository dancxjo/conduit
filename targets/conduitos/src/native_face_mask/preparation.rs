//! Host preparation of the exact checked Mask and its compositor custody.
use super::*;

enum Selection<'a> {
    Ordinary,
    OrdinaryCollection,
    Owner(
        &'a RemoteOwnerMaskRouteSeal,
        Option<&'a conduit_face::LookInfo>,
    ),
    Creche(
        conduit_face::CrecheComponentRecipe,
        conduit_face::ComponentVisualSelection,
        conduit_face::VisualTypography,
        bool,
    ),
}

impl NativeFaceMask {
    /// Narrow this prepared Mask before its first Show when the Host has no
    /// admitted local input provider. Face facts and owner actions stay intact.
    pub fn without_local_input(mut self) -> Result<Self, NativeFaceMaskError> {
        if self.scene.is_some() || self.session.is_some() {
            return Err(NativeFaceMaskError::Face);
        }
        self.local_input_available = false;
        Ok(self)
    }

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

    /// Select the existing Collection forwarding and Mask storage before Play.
    /// This does not enlarge Standard/owner/Crèche profiles or infer capacity
    /// from incoming Face data. The caller admits this workspace explicitly.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_collection(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
    ) -> Result<Self, NativeFaceMaskError> {
        if display_base_id != provider.entry.base_id
            || provider.entry.provider_generation == 0
            || provider.entry.lifecycle != conduit_core::BaseLifecycle::Ready
            || !provider.entry.resources.iter().any(|resource| {
                resource.class_id.as_str() == conduit_face::SHOW_RESOURCE_CLASS
                    && resource.capacity_units > 0
            })
        {
            return Err(NativeFaceMaskError::Plan);
        }
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            Selection::OrdinaryCollection,
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
        Self::prepare_with_owner_route_and_history_look(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            route,
            None,
        )
    }

    /// Independent typed history intent is rechecked against the actual offer
    /// and exact routed Plan. A Plan never supplies its own startup custody.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_owner_route_and_history_look(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        route: Option<&RemoteOwnerMaskRouteSeal>,
        history_look: Option<&conduit_face::LookInfo>,
    ) -> Result<Self, NativeFaceMaskError> {
        if history_look.is_some()
            && !route.is_some_and(|route| {
                matches!(
                    route.planned_mask.mask.plot_name.as_str(),
                    conduit_face::HISTORY_COMPONENT_MASK_ROOT
                        | conduit_face::HISTORY_LAYOUT_MASK_ROOT
                )
            })
        {
            return Err(NativeFaceMaskError::Plan);
        }
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            route.map_or(Selection::Ordinary, |route| {
                Selection::Owner(route, history_look)
            }),
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
        Self::prepare_creche_typographic(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            recipe,
            selection,
            conduit_face::VisualTypography::Editorial,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_creche_typographic(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        recipe: conduit_face::CrecheComponentRecipe,
        selection: conduit_face::ComponentVisualSelection,
        typography: conduit_face::VisualTypography,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            Selection::Creche(recipe, selection, typography, false),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_creche_navy(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
        display_base_id: HostBaseId,
        surface_id: &str,
        provider: &NativeSurfaceProvider,
        recipe: conduit_face::CrecheComponentRecipe,
        selection: conduit_face::ComponentVisualSelection,
        typography: conduit_face::VisualTypography,
    ) -> Result<Self, NativeFaceMaskError> {
        Self::prepare_selected(
            host_id,
            boot_id,
            offer_generation,
            build_id,
            display_base_id,
            surface_id,
            provider,
            Selection::Creche(recipe, selection, typography, true),
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
        let local_creche_preferences = matches!(&selection, Selection::Creche(..));
        let producer = producer::Producer::prepare(
            matches!(&selection, Selection::OrdinaryCollection),
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            build_id,
        )
        .map_err(NativeFaceMaskError::Producer)?;
        let mut component_configuration = None;
        let mask = match selection {
            Selection::Owner(route, history_look) => {
                let offer = crate::product_bases::native_mask_host_advertisement(
                    &host_id,
                    &boot_id,
                    offer_generation.0,
                    provider,
                );
                route
                    .validate_mask_host_offer(&offer)
                    .map_err(|_| NativeFaceMaskError::Plan)?;
                let selected = conduit_face::ComponentVisualSelection::from_todo_root(
                    &route.planned_mask.mask.plot_name,
                );
                let local = if matches!(
                    route.planned_mask.mask.plot_name.as_str(),
                    conduit_face::HISTORY_COMPONENT_MASK_ROOT
                        | conduit_face::HISTORY_LAYOUT_MASK_ROOT
                ) {
                    let (stage, configuration) = match history_look {
                        Some(look) => conduit_conduitos_mask_offer::prepare_history_component_stage_with_look_from_offer(&offer, look),
                        None => conduit_conduitos_mask_offer::prepare_history_component_stage_from_offer(&offer),
                    }.map_err(|_| NativeFaceMaskError::Plan)?;
                    component_configuration = Some(configuration);
                    stage
                } else if let Some((selection, recipe)) = selected {
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
            Selection::Creche(recipe, selection, typography, navy) => {
                let offer = crate::product_bases::native_mask_host_advertisement(
                    &host_id,
                    &boot_id,
                    offer_generation.0,
                    provider,
                );
                let prepare = if navy {
                    conduit_conduitos_mask_offer::prepare_navy_creche_stage_from_offer
                } else {
                    conduit_conduitos_mask_offer::prepare_typographic_creche_stage_from_offer
                };
                let (stage, configuration) = prepare(&offer, recipe, selection, typography)
                    .map_err(|_| NativeFaceMaskError::Plan)?;
                component_configuration = Some(configuration);
                stage
            }
            Selection::OrdinaryCollection => {
                let offer = conduit_conduitos_mask_offer::collection_native_host_advertisement(
                    &host_id,
                    &boot_id,
                    offer_generation.0,
                    &provider.entry,
                );
                conduit_conduitos_mask_offer::prepare_collection_stage_from_offer(&offer)
                    .map_err(|_| NativeFaceMaskError::Plan)?
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
        #[allow(unused_mut)]
        let mut compositor = NativeCompositor::admitted(
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
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if let Some(owner) = provider.display_owner.as_ref() {
            compositor
                .attach_display_owner(owner.clone())
                .map_err(NativeFaceMaskError::Compositor)?;
        }
        Ok(Self {
            producer,
            mask,
            compositor,
            display_base_id,
            surface_id: surface_id.into(),
            scene: None,
            input_refusal: None,
            session: None,
            publication: None,
            surface_admitted: false,
            interaction_admitted: true,
            local_input_available: true,
            local_creche_preferences,
            component_configuration,
            visual_look: None,
            visual_constraints: conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
            component_fallback: None,
            encounter_pending: false,
            history_scene: None,
            history_visible: false,
        })
    }
}
