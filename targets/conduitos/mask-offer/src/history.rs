//! The history Mask uses the existing initialized standard component renderer.
//! This accepts an actual current offer; it does not advertise or initialize one.
use super::{MaskOfferError, MaskStage, NativeMaskStorageProfile, configured};
use conduit_core::HostAdvertisement;
use conduit_face::{
    ConfiguredMaskConfiguration, HISTORY_COMPONENT_MASK_ROOT, HISTORY_COMPONENT_MASK_SOURCE,
};

pub fn prepare_history_component_stage_from_offer(
    host: &HostAdvertisement,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    configured::prepare_component_document(
        host,
        HISTORY_COMPONENT_MASK_SOURCE,
        HISTORY_COMPONENT_MASK_ROOT,
        NativeMaskStorageProfile::Standard,
    )
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
    configured::prepare_component_document_with_startup(
        host,
        HISTORY_COMPONENT_MASK_SOURCE,
        HISTORY_COMPONENT_MASK_ROOT,
        NativeMaskStorageProfile::Standard,
        Some(core::slice::from_ref(&input)),
    )
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
