//! Reviewed numerical algorithms offered for a source-sealed evaluation Kind.
use alloc::{format, vec};
use conduit_core::numeric_realization::{numeric_engine_artifact_id, NumericAlgorithm};
use conduit_core::*;

pub const NUMERIC_EVALUATION_ALGORITHMS: &[NumericAlgorithm] = &[
    NumericAlgorithm::OrderedCheckedI64V1,
    NumericAlgorithm::OrderedRationalV1,
    NumericAlgorithm::SymbolicRetentionV1,
    NumericAlgorithm::OrderedSoftwareBinary64V1,
    NumericAlgorithm::ExactComparisonV1,
    NumericAlgorithm::OrderedIeeeSoftwareBinary64V1,
];

pub fn numeric_evaluation_offer(
    kind: Kind,
    implementation: &str,
    profile: &str,
    algorithm: NumericAlgorithm,
) -> CapabilityOffer {
    let capability = format!("{implementation}/{}/{algorithm:?}", kind.kind_id.as_str());
    let mut offer = BackOfferBuilder::new(
        kind,
        Back {
            capability_id: capability.into(),
            execution_profile_id: profile.into(),
            implementation_id: implementation.into(),
            artifact_id: numeric_engine_artifact_id(),
            host_calls: vec![],
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                if algorithm == NumericAlgorithm::OrderedCheckedI64V1 {
                    65536
                } else {
                    1048576
                },
            )],
            authority_requirements: vec![],
        },
    )
    .build();
    let native = algorithm == NumericAlgorithm::OrderedCheckedI64V1;
    offer.realization_properties.push(
        NumericRealizationOffer {
            algorithm,
            budget: NumericExecutionBudget {
                storage_bytes: 1_048_576,
                work_steps: 10_000_000,
                limbs: if native { 1 } else { 128 },
                symbolic_nodes: if native { 0 } else { 128 },
            },
        }
        .configuration()
        .expect("reviewed finite numerical offer"),
    );
    offer
}

/// The derived Kind's semantic contract is preserved in the installed offer.
#[cfg(feature = "plot-catalog")]
pub fn numeric_evaluation_kind(
    gear: &conduit_plot::CheckedGear,
) -> Result<Kind, alloc::string::String> {
    conduit_plot::checked_gear_numeric_computation(gear)
        .map_err(|error| format!("numeric source: {error:?}"))?;
    let kind = Kind {
        kind_id: gear.kind_id.clone(),
        kind_contract_revision: gear.kind_contract_revision.clone(),
        startup_parameters: gear.startup_parameters.clone(),
        shorthand: gear.shorthand.clone(),
        inputs: gear.inputs.clone(),
        outputs: gear.outputs.clone(),
        configuration: gear.semantic_contract.configuration.clone(),
        semantic_laws: gear.semantic_contract.laws.clone(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    };
    kind.validate()
        .map_err(|error| format!("numeric Kind: {error:?}"))?;
    Ok(kind)
}
