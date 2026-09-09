//! Finite hosted std offers for the portable navigation waist.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PortDescriptor, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const NAVIGATION_STD_ARTIFACT: &str = "conduit-std-host/navigation@1";

pub const NAVIGATION_ROUTE_GRID4_PROFILE: &str = "std/navigation-route-grid4-orthogonal-hosted@1";
pub const NAVIGATION_ROUTE_GRID4_IMPLEMENTATION: &str =
    "std/kernel-navigation-route-grid4-orthogonal@1";
pub const NAVIGATION_ROUTE_GRID4_POSE_OPERATION: &str =
    "conduit.host/navigation-route-grid4-pose@1";
pub const NAVIGATION_ROUTE_GRID4_GOAL_OPERATION: &str =
    "conduit.host/navigation-route-grid4-goal@1";
pub const NAVIGATION_ROUTE_GRID4_TRAVERSABILITY_OPERATION: &str =
    "conduit.host/navigation-route-grid4-traversability@1";

pub const NAVIGATION_TIME_PARAMETERIZE_PROFILE: &str =
    "std/navigation-time-parameterize-fixed-hosted@1";
pub const NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION: &str =
    "std/kernel-navigation-time-parameterize-fixed@1";
pub const NAVIGATION_TIME_PARAMETERIZE_ROUTE_OPERATION: &str =
    "conduit.host/navigation-time-parameterize-route@1";

pub const NAVIGATION_LOCAL_CONTROL_PROFILE: &str = "std/navigation-local-control-bounded-hosted@1";
pub const NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION: &str =
    "std/kernel-navigation-local-control-bounded@1";
pub const NAVIGATION_LOCAL_CONTROL_POSE_OPERATION: &str =
    "conduit.host/navigation-local-control-pose@1";
pub const NAVIGATION_LOCAL_CONTROL_TRAJECTORY_OPERATION: &str =
    "conduit.host/navigation-local-control-trajectory@1";

pub fn navigation_std_offers() -> Vec<CapabilityOffer> {
    vec![
        navigation_offer(
            conduit_semantic_catalog::NAVIGATION_ROUTE_GRID4_KIND,
            "std-navigation-route-grid4-orthogonal",
            NAVIGATION_ROUTE_GRID4_PROFILE,
            NAVIGATION_ROUTE_GRID4_IMPLEMENTATION,
            &[
                NAVIGATION_ROUTE_GRID4_POSE_OPERATION,
                NAVIGATION_ROUTE_GRID4_GOAL_OPERATION,
                NAVIGATION_ROUTE_GRID4_TRAVERSABILITY_OPERATION,
            ],
        ),
        navigation_offer(
            conduit_semantic_catalog::NAVIGATION_TIME_PARAMETERIZE_KIND,
            "std-navigation-time-parameterize-fixed",
            NAVIGATION_TIME_PARAMETERIZE_PROFILE,
            NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION,
            &[NAVIGATION_TIME_PARAMETERIZE_ROUTE_OPERATION],
        ),
        navigation_offer(
            conduit_semantic_catalog::NAVIGATION_LOCAL_CONTROL_KIND,
            "std-navigation-local-control-bounded",
            NAVIGATION_LOCAL_CONTROL_PROFILE,
            NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION,
            &[
                NAVIGATION_LOCAL_CONTROL_POSE_OPERATION,
                NAVIGATION_LOCAL_CONTROL_TRAJECTORY_OPERATION,
            ],
        ),
    ]
}

fn navigation_offer(
    expected_kind: &str,
    capability: &str,
    profile: &str,
    implementation: &str,
    operation_contracts: &[&str],
) -> CapabilityOffer {
    let (kind, inputs, outputs) = navigation_contract(expected_kind);
    assert_eq!(
        inputs.len(),
        operation_contracts.len(),
        "each navigation input requires one exact host operation"
    );
    let host_operations = operation_contracts
        .iter()
        .map(|contract| HostOperationRequirement {
            contract_id: HostOperationContractId::from(*contract),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        })
        .collect();
    let max_queue_items = inputs.len() as u16;

    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: kind,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::NAVIGATION_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(NAVIGATION_STD_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations,
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items,
            max_queue_bytes: u32::from(max_queue_items)
                .saturating_mul(MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
        },
    }
}

fn navigation_contract(expected_kind: &str) -> (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>) {
    conduit_semantic_catalog::navigation_kind_contracts()
        .into_iter()
        .find(|(kind, _, _)| kind.as_str() == expected_kind)
        .unwrap_or_else(|| panic!("missing portable navigation contract for {expected_kind}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn offers_preserve_exact_portable_navigation_faces() {
        let offers = navigation_std_offers();
        let contracts = conduit_semantic_catalog::navigation_kind_contracts();
        assert_eq!(offers.len(), contracts.len());

        for (offer, (kind, inputs, outputs)) in offers.iter().zip(contracts) {
            assert_eq!(offer.kind_id, kind);
            assert_eq!(offer.inputs, inputs);
            assert_eq!(offer.outputs, outputs);
            assert_eq!(
                offer.kind_contract_revision.as_str(),
                conduit_semantic_catalog::NAVIGATION_REVISION
            );
        }
    }

    #[test]
    fn every_input_has_one_finite_authority_free_host_operation() {
        for offer in navigation_std_offers() {
            assert_eq!(offer.host_operations.len(), offer.inputs.len());
            assert!(offer.resource_requirements.is_empty());
            assert!(offer.authority_requirements.is_empty());
            assert_eq!(offer.limits.max_queue_items as usize, offer.inputs.len());
            assert_eq!(
                offer.limits.max_queue_bytes,
                offer.inputs.len() as u32 * MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            );
            for operation in &offer.host_operations {
                assert_eq!(operation.target_kind.as_ref(), Some(&offer.kind_id));
                assert_eq!(operation.maximum_in_flight, 1);
                assert_eq!(
                    operation.maximum_input_bytes,
                    MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                );
                assert_eq!(
                    operation.maximum_output_bytes,
                    MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                );
            }
        }
    }

    #[test]
    fn each_navigation_kind_has_distinct_profile_and_implementation_identity() {
        let offers = navigation_std_offers();
        let profiles: BTreeSet<_> = offers
            .iter()
            .map(|offer| offer.implementation.execution_profile_id.as_str())
            .collect();
        let implementations: BTreeSet<_> = offers
            .iter()
            .map(|offer| offer.implementation.implementation_id.as_str())
            .collect();
        assert_eq!(profiles.len(), offers.len());
        assert_eq!(implementations.len(), offers.len());
        assert!(offers.iter().all(|offer| {
            offer.implementation.execution_profile_id.as_str()
                != offer.implementation.implementation_id.as_str()
        }));
    }
}
