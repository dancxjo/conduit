//! Stable std entrance to its pure package-owned Home realization descriptor.
#[cfg(test)]
pub(crate) use conduit_std_offers::ambient_home_std_offer as offer;
pub(crate) use conduit_std_offers::{
    AMBIENT_HOME_IMPLEMENTATION as IMPLEMENTATION, ambient_home_workspace_bytes as workspace_bytes,
};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_retains_exact_semantics_and_independent_call_identities() {
        let offered = offer(crate::ambient_home_offer::workspace_bytes().unwrap()).unwrap();
        let kind = conduit_semantic_catalog::ambient_home_contributor_kind().unwrap();
        assert_eq!(offered.kind_id, kind.kind_id);
        assert_eq!(offered.kind_contract_revision, kind.kind_contract_revision);
        assert_eq!(offered.inputs, kind.inputs);
        assert_eq!(offered.outputs, kind.outputs);
        assert_eq!(offered.limits, kind.limits);
        assert_eq!(offered.host_calls.len(), 2);
        assert_ne!(
            offered.host_calls[0].contract_id,
            offered.host_calls[1].contract_id
        );
        assert!(
            offered
                .host_calls
                .iter()
                .all(|call| call.maximum_input_bytes == 8192
                    && call.maximum_output_bytes == 0
                    && call.maximum_in_flight == 1)
        );
        assert_eq!(offered.inputs.len(), 1);
        assert!(offered.outputs.is_empty());
    }
    #[test]
    fn workspace_is_an_explicit_realization_requirement() {
        assert!(offer(0).is_err());
        assert_ne!(
            offer(crate::ambient_home_offer::workspace_bytes().unwrap())
                .unwrap()
                .resource_requirements,
            offer(crate::ambient_home_offer::workspace_bytes().unwrap() + 1)
                .unwrap()
                .resource_requirements
        );
    }
}
