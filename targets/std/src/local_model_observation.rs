//! Current local-model realization truth published by an initialized std Host.

use crate::StdHost;
use conduit_core::{PoolRealizationEnvelope, PoolRealizationObservation, SignId};

impl StdHost {
    pub fn observe_local_model_pool_realization(
        &self,
        realization: &PoolRealizationEnvelope,
        provider_sign_id: SignId,
        resource_sign_ids: &[SignId],
    ) -> Result<PoolRealizationObservation, String> {
        if provider_sign_id.as_str().is_empty() {
            return Err("local-model provider observation Sign identity is empty".into());
        }
        let advertisement = self.advertisement();
        if realization.host_id != advertisement.host_id
            || realization.boot_id != advertisement.boot_id
            || realization.offer_generation != advertisement.offer_generation
        {
            return Err("local-model realization is stale for this Host advertisement".into());
        }
        let capability = advertisement
            .capabilities
            .iter()
            .find(|candidate| candidate.capability_id == realization.capability_id)
            .ok_or_else(|| {
                "local-model realization capability is not currently offered".to_string()
            })?;
        if capability.implementation.implementation_id != realization.implementation_id
            || capability.implementation.artifact_id != realization.artifact_id
            || capability.implementation.implementation_id.as_str()
                != conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        {
            return Err("local-model realization executable identity changed".into());
        }
        let adapter = self
            .local_model
            .as_deref()
            .ok_or_else(|| "local-model provider is not initialized".to_string())?;
        let resources = self.kernel_resources.observe_bindings(
            advertisement,
            &realization.resources,
            resource_sign_ids,
        )?;
        let observation = PoolRealizationObservation {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation.implementation_id.clone(),
            artifact_id: capability.implementation.artifact_id.clone(),
            health: adapter.current_pool_health(),
            sign_id: provider_sign_id,
            resources,
        };
        if !observation.is_current_for(realization) {
            return Err("local-model observation does not bind the sealed realization".into());
        }
        Ok(observation)
    }
}
