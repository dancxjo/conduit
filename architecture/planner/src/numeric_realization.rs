//! Numerical proof gates precede preferences and resource-cost selection.
use crate::prelude::*;
use crate::PlannerError;
use conduit_core::numeric_projection::{
    numeric_realization_offer_profile, numeric_witness_configuration, NumericRealizationOffer,
    NumericRequirement,
};
use conduit_core::numeric_realization::{NumericEngineRefusal, VerifiedNumericWitness};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityOffer, ConfigurationValue, GearId,
    HostAdvertisement, HostId, KindSemanticLaw, OfferGeneration, StructuredConfigurationValue,
};
use conduit_plot::CheckedGear;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericCandidateEvidence {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub artifact_id: ArtifactId,
    pub algorithm: Option<conduit_core::numeric_realization::NumericAlgorithm>,
    pub refusal: NumericEngineRefusal,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericRealizationUnsatisfied {
    pub gear_id: GearId,
    pub requirement_identity: [u8; 32],
    pub candidates: Vec<NumericCandidateEvidence>,
}
fn configuration<'a>(
    gear: &'a CheckedGear,
    key: &str,
) -> Result<&'a StructuredConfigurationValue, PlannerError> {
    gear.configuration
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| match &entry.value {
            ConfigurationValue::Structured(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| PlannerError::InvalidNumericRequirement(gear.gear_id.clone()))
}
fn requirement(gear: &CheckedGear) -> Result<Option<NumericRequirement>, PlannerError> {
    let mut result = None;
    for law in &gear.semantic_contract.laws {
        if let KindSemanticLaw::RealizationRequirement {
            property_profile,
            configuration_key,
        } = law
        {
            if *property_profile != numeric_realization_offer_profile() {
                continue;
            }
            if result.is_some() {
                return Err(PlannerError::InvalidNumericRequirement(
                    gear.gear_id.clone(),
                ));
            }
            result = Some(
                NumericRequirement::from_configuration(configuration(gear, configuration_key)?)
                    .map_err(|_| PlannerError::InvalidNumericRequirement(gear.gear_id.clone()))?,
            );
        }
    }
    Ok(result)
}
fn witness(
    gear: &CheckedGear,
    offer: &CapabilityOffer,
    requirement: &NumericRequirement,
) -> Result<(Vec<VerifiedNumericWitness>, bool), NumericEngineRefusal> {
    if offer.implementation.artifact_id
        != conduit_core::numeric_realization::numeric_engine_artifact_id()
    {
        return Err(NumericEngineRefusal::ClaimMismatch);
    }
    let tensor = gear
        .kind_id
        .as_str()
        .starts_with("conduitese/tensor/evaluate/");
    let graphs = if tensor {
        let bundle = conduit_plot::checked_gear_tensor_computations(gear)
            .map_err(|_| NumericEngineRefusal::ClaimMismatch)?;
        if &bundle.requirement != requirement {
            return Err(NumericEngineRefusal::ClaimMismatch);
        }
        bundle.coordinates
    } else {
        vec![conduit_plot::checked_gear_numeric_computation(gear)
            .map_err(|_| NumericEngineRefusal::ClaimMismatch)?]
    };
    let property = offer
        .realization_properties
        .iter()
        .find(|property| *property.profile() == numeric_realization_offer_profile())
        .ok_or(NumericEngineRefusal::UnsupportedRepresentation)?;
    let declared = NumericRealizationOffer::from_configuration(property)?;
    let verified = if tensor {
        declared.certify_tensor(&graphs, requirement)?
    } else {
        vec![declared.certify(&graphs[0], requirement)?]
    };
    let required = verified.iter().zip(&graphs).try_fold(
        if tensor {
            conduit_core::NUMERIC_TENSOR_MATERIALIZATION_RESERVATION_BYTES
        } else {
            0
        },
        |total, (witness, graph)| {
            total
                .checked_add(witness.required_memory_reservation_bytes(graph)?)
                .ok_or(NumericEngineRefusal::Arithmetic(
                    conduit_core::NumericRefusal::StorageBudget,
                ))
        },
    )?;
    let reserved = offer
        .resource_requirements
        .iter()
        .filter(|resource| {
            resource.class_id.as_str() == conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS
        })
        .try_fold(0u32, |total, resource| total.checked_add(resource.units))
        .ok_or(NumericEngineRefusal::Arithmetic(
            conduit_core::NumericRefusal::StorageBudget,
        ))?;
    if reserved < required {
        return Err(NumericEngineRefusal::Arithmetic(
            conduit_core::NumericRefusal::StorageBudget,
        ));
    }
    Ok((verified, tensor))
}
pub(crate) fn filter_candidates<T, F>(
    gear: &CheckedGear,
    candidates: Vec<T>,
    fields: F,
) -> Result<Vec<T>, PlannerError>
where
    F: for<'a> Fn(&'a T) -> (&'a HostAdvertisement, &'a CapabilityOffer),
{
    let Some(requirement) = requirement(gear)? else {
        return Ok(candidates);
    };
    if candidates.len() > crate::MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(
            "numerical candidate evidence bound".into(),
        ));
    }
    let mut eligible = Vec::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        let (host, offer) = fields(&candidate);
        if !conduit_core::valid_realization_properties(&offer.realization_properties) {
            return Err(PlannerError::InvalidRealizationProperties(
                offer.capability_id.clone(),
            ));
        }
        match witness(gear, offer, &requirement) {
            Ok(_) => eligible.push(candidate),
            Err(refusal) => rejected.push(NumericCandidateEvidence {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                capability_id: offer.capability_id.clone(),
                artifact_id: offer.implementation.artifact_id.clone(),
                algorithm: offer
                    .realization_properties
                    .iter()
                    .find(|p| *p.profile() == numeric_realization_offer_profile())
                    .and_then(|p| NumericRealizationOffer::from_configuration(p).ok())
                    .map(|p| p.algorithm),
                refusal,
            }),
        }
    }
    if eligible.is_empty() {
        return Err(PlannerError::NumericRealizationUnsatisfied(
            alloc::boxed::Box::new(NumericRealizationUnsatisfied {
                gear_id: gear.gear_id.clone(),
                requirement_identity: requirement
                    .identity()
                    .map_err(|_| PlannerError::InvalidNumericRequirement(gear.gear_id.clone()))?,
                candidates: rejected,
            }),
        ));
    }
    Ok(eligible)
}
pub(crate) fn sealed_properties(
    gear: &CheckedGear,
    offer: &CapabilityOffer,
) -> Result<Vec<StructuredConfigurationValue>, PlannerError> {
    let mut properties = offer.realization_properties.clone();
    if let Some(requirement) = requirement(gear)? {
        let verified = witness(gear, offer, &requirement)
            .map_err(|_| PlannerError::InvalidNumericRequirement(gear.gear_id.clone()))?;
        properties.push(
            (if verified.1 {
                conduit_core::numeric_tensor_witness_configuration(&verified.0)
            } else {
                numeric_witness_configuration(&verified.0[0])
            })
            .map_err(|_| PlannerError::InvalidNumericRequirement(gear.gear_id.clone()))?,
        );
        properties.sort_by(|a, b| a.profile().cmp(b.profile()));
    }
    if !conduit_core::valid_realization_properties(&properties) {
        return Err(PlannerError::InvalidRealizationProperties(
            offer.capability_id.clone(),
        ));
    }
    Ok(properties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{numeric_realization::NumericAlgorithm, NumericExecutionBudget};
    fn fixture(
        expression: &str,
        representation: &str,
        accuracy: &str,
    ) -> (CheckedGear, HostAdvertisement) {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        conduit_semantic_catalog::install_mathematical_kinds(&mut startup, &mut profile).unwrap();
        let source = format!("plot main () {{ math/evaluate(value = {expression}, requirement = {{ semantics: mathematical(empty), representation: {representation}(empty), rounding: nearestEven(empty), accuracy: {accuracy}, budget: {{ storageBytes: 1048576, workSteps: 100000, limbs: 128, symbolicNodes: 128 }}, parameters: [], reproducible: true, exceptions: {{ invalid: false, divisionByZero: false, overflow: false, underflow: false, inexact: true }} }}) }}.");
        let syntax = conduit_plot::parse_syntax_document(&source);
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_plot::expand_canonical_plot(&checked, "main", &profile).unwrap();
        let gear = expanded.gears[0].clone();
        let mut host = crate::tests::host();
        let mut offer = host.capabilities[0].clone();
        offer.implementation.artifact_id =
            conduit_core::numeric_realization::numeric_engine_artifact_id();
        offer.kind_id = gear.kind_id.clone();
        offer.kind_contract_revision = gear.kind_contract_revision.clone();
        offer.semantic_contract = gear.semantic_contract.clone();
        offer.startup_parameters = gear.startup_parameters.clone();
        offer.shorthand = gear.shorthand.clone();
        offer.inputs = gear.inputs.clone();
        offer.outputs = gear.outputs.clone();
        offer.host_calls.clear();
        offer.resource_requirements.clear();
        offer.authority_requirements.clear();
        host.capabilities = vec![offer];
        (gear, host)
    }
    fn declare(host: &mut HostAdvertisement, algorithm: NumericAlgorithm) {
        host.capabilities[0]
            .resource_requirements
            .push(conduit_core::ResourceRequirement {
                class_id: conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS.into(),
                units: 1048576,
                protected_role: None,
                compute: None,
                content: None,
            });
        host.capabilities[0].realization_properties = vec![NumericRealizationOffer {
            algorithm,
            budget: NumericExecutionBudget {
                storage_bytes: 1048576,
                work_steps: 100000,
                limbs: 128,
                symbolic_nodes: 128,
            },
        }
        .configuration()
        .unwrap()];
    }
    #[test]
    fn evidence_and_preparation_memory_are_required_before_cost() {
        let (gear, mut host) = fixture("6 * 7", "automatic", "exact(empty)");
        declare(&mut host, NumericAlgorithm::OrderedCheckedI64V1);
        assert!(witness(
            &gear,
            &host.capabilities[0],
            &requirement(&gear).unwrap().unwrap()
        )
        .is_ok());
        host.capabilities[0].resource_requirements.clear();
        assert_eq!(
            witness(
                &gear,
                &host.capabilities[0],
                &requirement(&gear).unwrap().unwrap()
            )
            .unwrap_err(),
            NumericEngineRefusal::Arithmetic(conduit_core::NumericRefusal::StorageBudget)
        );
    }
    #[test]
    fn original_order_cancellation_refuses_before_cost_or_play() {
        let (gear, mut host) = fixture(
            "(10000000000000000 + 1) - 10000000000000000",
            "binary64",
            "exact(empty)",
        );
        declare(&mut host, NumericAlgorithm::OrderedSoftwareBinary64V1);
        let error = filter_candidates(&gear, vec![(&host, &host.capabilities[0])], |candidate| {
            (candidate.0, candidate.1)
        })
        .unwrap_err();
        let PlannerError::NumericRealizationUnsatisfied(evidence) = error else {
            panic!("typed numeric rejection expected");
        };
        assert_eq!(evidence.candidates.len(), 1);
        assert_eq!(
            evidence.candidates[0].refusal,
            NumericEngineRefusal::AccuracyUnproved
        );
    }
    #[test]
    fn selected_witness_is_sealed_and_requirement_changes_refuse() {
        let (gear, mut host) = fixture("0.1 + 0.2", "rational", "exact(empty)");
        declare(&mut host, NumericAlgorithm::OrderedRationalV1);
        let properties = sealed_properties(&gear, &host.capabilities[0]).unwrap();
        assert_eq!(properties.len(), 2);
        assert!(conduit_core::valid_realization_properties(&properties));
        let graph = conduit_plot::checked_gear_numeric_computation(&gear).unwrap();
        let request = requirement(&gear).unwrap().unwrap();
        let property = properties
            .iter()
            .find(|p| *p.profile() == conduit_core::numeric_realization_property_profile())
            .unwrap();
        conduit_core::verify_numeric_witness_configuration(property, &graph, &request).unwrap();
        let (changed, _) = fixture("0.1 + 0.2", "binary64", "exact(empty)");
        assert!(sealed_properties(&changed, &host.capabilities[0]).is_err());
    }
    #[test]
    fn automatic_keeps_checked_integer_eligible_and_rejects_overflow() {
        let (gear, mut host) = fixture("6 * 7", "automatic", "exact(empty)");
        declare(&mut host, NumericAlgorithm::OrderedCheckedI64V1);
        assert!(
            filter_candidates(&gear, vec![(&host, &host.capabilities[0])], |c| (c.0, c.1)).is_ok()
        );
        let (large, mut large_host) =
            fixture("9223372036854775807 + 1", "automatic", "exact(empty)");
        declare(&mut large_host, NumericAlgorithm::OrderedCheckedI64V1);
        assert!(filter_candidates(
            &large,
            vec![(&large_host, &large_host.capabilities[0])],
            |c| (c.0, c.1)
        )
        .is_err());
        declare(&mut large_host, NumericAlgorithm::OrderedRationalV1);
        assert!(filter_candidates(
            &large,
            vec![(&large_host, &large_host.capabilities[0])],
            |c| (c.0, c.1)
        )
        .is_ok());
    }
    #[test]
    fn eligibility_precedes_cost_for_two_consumers_of_one_computation() {
        let (exact, mut exact_host) = fixture("0.1 + 0.2", "automatic", "exact(empty)");
        let (approximate, mut approximate_host) =
            fixture("0.1 + 0.2", "automatic", "absolute(0.000000000001)");
        assert_eq!(
            conduit_plot::checked_gear_numeric_computation(&exact).unwrap(),
            conduit_plot::checked_gear_numeric_computation(&approximate).unwrap()
        );
        let class = conduit_core::ResourceClassId::from("test/numeric-work-reservation");
        let configure = |host: &mut HostAdvertisement| {
            declare(host, NumericAlgorithm::OrderedRationalV1);
            let mut fast = host.capabilities[0].clone();
            fast.capability_id = CapabilityId::from("test/fast-binary64");
            fast.realization_properties = vec![NumericRealizationOffer {
                algorithm: NumericAlgorithm::OrderedSoftwareBinary64V1,
                budget: NumericExecutionBudget {
                    storage_bytes: 1048576,
                    work_steps: 100000,
                    limbs: 128,
                    symbolic_nodes: 128,
                },
            }
            .configuration()
            .unwrap()];
            let resource = |units| conduit_core::ResourceRequirement {
                class_id: class.clone(),
                units,
                protected_role: None,
                compute: None,
                content: None,
            };
            host.capabilities[0]
                .resource_requirements
                .push(resource(100));
            fast.resource_requirements.push(resource(1));
            host.capabilities.push(fast);
        };
        configure(&mut exact_host);
        configure(&mut approximate_host);
        let policy = crate::RealizationPolicy {
            preferences: vec![crate::RealizationPreference::MinimizeResourceUnits(class)],
        };
        let hard = crate::HardRealizationRequirements::default();
        let exact_choice =
            crate::select_realization_with_policy(&exact, &[exact_host.clone()], &hard, &policy)
                .unwrap();
        assert_eq!(
            exact_choice.capability_id,
            exact_host.capabilities[0].capability_id
        );
        let approximate_choice = crate::select_realization_with_policy(
            &approximate,
            &[approximate_host.clone()],
            &hard,
            &policy,
        )
        .unwrap();
        assert_eq!(
            approximate_choice.capability_id,
            approximate_host.capabilities[1].capability_id
        );
    }
    #[test]
    fn algorithm_declaration_cannot_certify_an_unreviewed_artifact() {
        let (gear, mut host) = fixture("6 * 7", "automatic", "exact(empty)");
        declare(&mut host, NumericAlgorithm::OrderedCheckedI64V1);
        host.capabilities[0].implementation.artifact_id =
            ArtifactId::from("test/unrelated-numeric-artifact");
        let error = filter_candidates(&gear, vec![(&host, &host.capabilities[0])], |candidate| {
            (candidate.0, candidate.1)
        })
        .unwrap_err();
        let PlannerError::NumericRealizationUnsatisfied(evidence) = error else {
            panic!("typed artifact rejection expected");
        };
        assert_eq!(
            evidence.candidates[0].refusal,
            NumericEngineRefusal::ClaimMismatch
        );
    }
}
