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
