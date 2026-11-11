//! Preparation of the installed model and exact large typed State realizations.
use crate::{
    hosted_local_model::HostedLocalModelAdapter, StdHost, StdHostComposition, StdHostConfig,
};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
impl StdHost {
    /// Admit the existing model provider plus one exact 64 KiB typed State
    /// specialization. Storage is Play-lived; this grants no durable residence,
    /// cross-Play transfer, extra effects, or arbitrary implementation injection.
    pub fn new_with_local_model_and_large_state(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn HostedLocalModelAdapter>,
        type_name: &str,
        value_type: &StructuredInfoType,
        initial: &StructuredInfoValue,
    ) -> Result<Self, String> {
        let state = conduit_std_offers::state_value_large_std_offer(type_name, value_type, initial)
            .map_err(|error| format!("large typed State offer: {error:?}"))?;
        Self::new_with_local_model_capabilities(config, composition, adapter, vec![state])
    }
}
