//! The history Mask uses the existing initialized standard component renderer.
//! This accepts an actual current offer; it does not advertise or initialize one.
use super::{MaskOfferError, MaskStage, NativeMaskStorageProfile, configured};
use conduit_core::HostAdvertisement;
use conduit_face::{
    ConfiguredMaskConfiguration, HISTORY_COMPONENT_MASK_ROOT, HISTORY_COMPONENT_MASK_SOURCE,
    HISTORY_LAYOUT_MASK_ROOT, HISTORY_LAYOUT_MASK_SOURCE,
};

pub fn prepare_history_component_stage_from_offer(
    host: &HostAdvertisement,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    prepare_history_document(host, None)
}

/// Supply a complete typed Look through the checked Root invocation before Plan.
/// The actual offered Back, Source recipe and preparation gates are unchanged.
pub fn prepare_history_component_stage_with_look_from_offer(
    host: &HostAdvertisement,
    look: &conduit_face::LookInfo,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    use conduit_plot::rust_binding::NativeRustBinding;
    let value = look
        .clone()
        .into_structured()
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let input = conduit_plot::CheckedStartupBinding {
        name: "look".into(),
        value_type: "LookInfo".into(),
        value: conduit_plot::CanonicalStartupValue::Structured(
            conduit_plot::CanonicalStructuredStartupValue::from_owned_checked_value(value),
        ),
    };
    prepare_history_document(host, Some(core::slice::from_ref(&input)))
}

fn prepare_history_document(
    host: &HostAdvertisement,
    inputs: Option<&[conduit_plot::CheckedStartupBinding]>,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    // Presence selects this exact deployment contract. A malformed or stale
    // layout offer refuses in ordinary planning; never fabricate the old Back.
    if host
        .capabilities
        .iter()
        .any(|offer| offer.capability_id.as_str() == super::NATIVE_LAYOUT_COMPONENT_CAPABILITY)
    {
        super::layout_configured::prepare_layout_component_document_with_startup(
            host,
            HISTORY_LAYOUT_MASK_SOURCE,
            HISTORY_LAYOUT_MASK_ROOT,
            NativeMaskStorageProfile::Standard,
            inputs,
        )
    } else {
        configured::prepare_component_document_with_startup(
            host,
            HISTORY_COMPONENT_MASK_SOURCE,
            HISTORY_COMPONENT_MASK_ROOT,
            NativeMaskStorageProfile::Standard,
            inputs,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{ComponentRecipeInfo, HISTORY_COMPONENT_RECIPE_ID};
    use conduit_plot::rust_binding::NativeRustBinding;

    #[test]
    fn checked_history_source_retains_exact_configuration_on_the_offered_back() {
        // CPU offered-Host fixture only. Production callers supply their actual
        // independently observed offer and initialized surface/renderer custody.
        let host = crate::renderer_host(
            crate::Adapter::Native,
            &"host/history-cpu-fixture".into(),
            &"boot/history-cpu-fixture".into(),
            1,
            None,
        );
        let (stage, configuration) = prepare_history_component_stage_from_offer(&host).unwrap();
        assert!(conduit_core::verify_plan(&stage.planned_mask.plan));
        assert_eq!(
            stage.planned_mask.mask.plot_name,
            HISTORY_COMPONENT_MASK_ROOT
        );
        assert_eq!(configuration.host_id, host.host_id);
        assert_eq!(configuration.boot_id, host.boot_id);
        let recipe = ComponentRecipeInfo::from_structured(
            conduit_core::StructuredInfoValue::from_canonical_bytes(
                configuration.recipe.canonical_value(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(recipe.identity(), HISTORY_COMPONENT_RECIPE_ID);
        assert_eq!(recipe.regions().len(), 1);
        let mut absent = host.clone();
        absent
            .capabilities
            .retain(|back| back.capability_id.as_str() != "renderer-conduitos-component");
        assert!(prepare_history_component_stage_from_offer(&absent).is_err());
        let mut stale = host;
        stale.offer_generation.0 = 0;
        assert!(prepare_history_component_stage_from_offer(&stale).is_err());
    }
}

#[cfg(test)]
mod quantity_tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    use conduit_face::*;

    // Exact semantic recipe-binding fixture, not recovered recording evidence.
    // This crate admits Mask plans; native Mask Play/scanout lives in ConduitOS.
    fn face() -> Face {
        use FacePropertyValue as V;
        let subject = "environment-history";
        let properties = [
            ("schema", V::Text("environment/history@1".into())),
            ("clock", V::Text("producer-boot-ms".into())),
            ("producer-host", V::Identity("fixture/sensor".into())),
            ("producer-boot", V::Identity("fixture/boot".into())),
            ("producer-plan", V::Identity("fixture/plan".into())),
            ("producer-play", V::Identity("fixture/play".into())),
            ("visible-samples", V::Count(0)),
            ("samples-outside-view", V::Count(0)),
        ]
        .into_iter()
        .map(|(name, value)| FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        })
        .collect();
        Face::new_with_semantics(
            1,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![FaceSubject {
                identity: subject.into(),
                role: FaceRole::Collection,
                name: "Recorded history".into(),
            }],
            vec![],
            properties,
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn three_units_use_one_source_recipe_face_and_offered_back_with_exact_new_plans() {
        let host = crate::renderer_host(
            crate::Adapter::Native,
            &"host/history-cpu-fixture".into(),
            &"boot/history-cpu-fixture".into(),
            1,
            None,
        );
        let (default, authored) = prepare_history_component_stage_from_offer(&host).unwrap();
        let base = authored.requested_look().unwrap();
        let (explicit_default, explicit) =
            prepare_history_component_stage_with_look_from_offer(&host, &base).unwrap();
        assert_eq!(
            default.planned_mask.plan,
            explicit_default.planned_mask.plan
        );
        assert_eq!(authored.look, explicit.look);
        let face = face();
        let original = face.clone();
        let mut ids = Vec::new();
        for unit in [
            LookTemperatureUnit::Celsius,
            LookTemperatureUnit::Kelvin,
            LookTemperatureUnit::Fahrenheit,
        ] {
            let requested = LookInfo::new(
                base.generative().clone(),
                base.identity().into(),
                LookQuantityFacetOption::some(unit).unwrap(),
                base.spoken().clone(),
                3,
                base.visual().clone(),
            )
            .unwrap();
            let (stage, configuration) =
                prepare_history_component_stage_with_look_from_offer(&host, &requested).unwrap();
            assert!(conduit_core::verify_plan(&stage.planned_mask.plan));
            assert_eq!(configuration.requested_look().unwrap(), requested);
            assert_eq!(configuration.component, authored.component);
            assert_eq!(configuration.recipe, authored.recipe);
            assert_eq!(configuration.placement_id, authored.placement_id);
            assert_eq!(configuration.implementation_id, authored.implementation_id);
            assert_eq!(
                stage.planned_mask.plan.source_document_id,
                default.planned_mask.plan.source_document_id
            );
            assert_eq!(
                stage.planned_mask.plan.checked_plot_id,
                default.planned_mask.plan.checked_plot_id
            );
            let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
                configuration.recipe.canonical_value(),
            )
            .unwrap();
            prepare_component_recipe(recipe, &face).unwrap();
            let admitted = admit_visual_look(
                &requested,
                VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: 100,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            assert_eq!(admitted.selected.quantity(), requested.quantity());
            assert_eq!(admitted.selected.visual(), base.visual());
            assert!(ids.iter().all(|id| id != &stage.planned_mask.plan.plan_id));
            ids.push(stage.planned_mask.plan.plan_id.clone());
            assert_eq!(face, original);
        }
        let mut absent = host.clone();
        absent
            .capabilities
            .retain(|b| b.capability_id.as_str() != "renderer-conduitos-component");
        assert!(prepare_history_component_stage_with_look_from_offer(&absent, &base).is_err());
        let mut stale = host;
        stale.offer_generation.0 = 0;
        assert!(prepare_history_component_stage_with_look_from_offer(&stale, &base).is_err());
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    use conduit_core::*;
    use conduit_face::*;
    use conduit_plot::rust_binding::NativeRustBinding;

    #[test]
    fn deployed_layout_offer_selects_its_checked_source_for_default_and_three_unit_inputs() {
        // CPU Base/Host offer fixture. The product uses this same deployment
        // constructor with actual independently retained display custody.
        let provider = BaseProviderEntry {
            base_id: "fixture/history/layout".into(),
            provider_instance_id: "fixture/history/provider".into(),
            provider_generation: 1,
            implementation_id: "conduitos/framebuffer@1".into(),
            mechanism_family: "conduitos.base/framebuffer@1".into(),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![],
            resources: vec![resource_offer(
                "fixture/history/surface",
                SHOW_RESOURCE_CLASS,
                1,
            )],
        };
        let host = crate::native_layout_host_advertisement(
            &"host/history-layout-cpu".into(),
            &"boot/history-layout-cpu".into(),
            1,
            &provider,
            NativeMaskStorageProfile::Standard,
        );
        assert!(
            !host
                .capabilities
                .iter()
                .any(|b| b.capability_id.as_str() == "renderer-conduitos-component")
        );
        let (default, configured) = prepare_history_component_stage_from_offer(&host).unwrap();
        assert_eq!(
            default.planned_mask.mask.plot_name,
            HISTORY_LAYOUT_MASK_ROOT
        );
        assert!(configured.layout.is_some());
        assert!(
            default
                .planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|f| &f.placements)
                .any(|p| p.capability_id.as_str() == crate::NATIVE_LAYOUT_COMPONENT_CAPABILITY)
        );
        let layout = ComponentLayoutInfo::from_structured(
            StructuredInfoValue::from_canonical_bytes(
                configured.layout.as_ref().unwrap().canonical_value(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(layout.recipe_identity(), HISTORY_COMPONENT_RECIPE_ID);
        let base = configured.requested_look().unwrap();
        let (same, _) = prepare_history_component_stage_with_look_from_offer(&host, &base).unwrap();
        assert_eq!(same.planned_mask.plan, default.planned_mask.plan);
        let mut ids = Vec::new();
        for unit in [
            LookTemperatureUnit::Celsius,
            LookTemperatureUnit::Kelvin,
            LookTemperatureUnit::Fahrenheit,
        ] {
            let look = LookInfo::new(
                base.generative().clone(),
                base.identity().into(),
                LookQuantityFacetOption::some(unit).unwrap(),
                base.spoken().clone(),
                3,
                base.visual().clone(),
            )
            .unwrap();
            let (stage, selected) =
                prepare_history_component_stage_with_look_from_offer(&host, &look).unwrap();
            assert_eq!(selected.requested_look().unwrap(), look);
            assert_eq!(selected.recipe, configured.recipe);
            assert_eq!(selected.layout, configured.layout);
            assert_eq!(
                stage.planned_mask.plan.source_document_id,
                default.planned_mask.plan.source_document_id
            );
            assert!(ids.iter().all(|id| id != &stage.planned_mask.plan.plan_id));
            ids.push(stage.planned_mask.plan.plan_id);
        }
        let mut malformed = host;
        malformed
            .capabilities
            .iter_mut()
            .find(|b| b.capability_id.as_str() == crate::NATIVE_LAYOUT_COMPONENT_CAPABILITY)
            .unwrap()
            .kind_contract_revision = "wrong/layout-contract".into();
        assert!(prepare_history_component_stage_from_offer(&malformed).is_err());
        assert!(prepare_history_component_stage_with_look_from_offer(&malformed, &base).is_err());
    }
}
