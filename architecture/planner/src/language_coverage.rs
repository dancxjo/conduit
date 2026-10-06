//! Domain-owned language facts gate realization before resource/policy selection.
//! No language names, prefix matching, provider inference, detection or retry.
use crate::prelude::*;
use crate::{PlannerError, MAXIMUM_REALIZATION_DECISION_RECORDS};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityOffer, HostAdvertisement, HostId, KindSemanticLaw,
    OfferGeneration,
};
use conduit_language::{
    admit_language_coverage, language_coverage_profile, validate_language_request,
    LanguageCoverage, LanguageCoverageRefusal, LanguageCoverageUse, LanguageRequest,
};
use conduit_plot::{rust_binding::NativeRustBinding, CheckedGear};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverageUnsatisfied {
    pub gear_id: conduit_core::GearId,
    pub requirements: Vec<LanguageCoverageRequirement>,
    pub candidates: Vec<LanguageCoverageCandidateEvidence>,
    pub reason: Option<LanguageCoverageRefusal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverageRequirement {
    pub configuration_key: String,
    pub request: LanguageRequest,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverageCheck {
    pub requirement: LanguageCoverageRequirement,
    pub coverage_revision: Option<String>,
    pub variety_sensitive: Option<bool>,
    pub result: Result<LanguageCoverageUse, LanguageCoverageRefusal>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverageCandidateEvidence {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub artifact_id: ArtifactId,
    pub checks: Vec<LanguageCoverageCheck>,
}

pub(crate) fn requirements(
    gear: &CheckedGear,
) -> Result<Vec<LanguageCoverageRequirement>, PlannerError> {
    let mut requests = Vec::new();
    for law in &gear.semantic_contract.laws {
        let KindSemanticLaw::RealizationRequirement {
            property_profile,
            configuration_key,
        } = law
        else {
            continue;
        };
        if *property_profile != language_coverage_profile() {
            return Err(PlannerError::UnsupportedRealizationRequirement {
                gear_id: gear.gear_id.clone(),
                property_profile: property_profile.clone(),
            });
        }
        let request = gear
            .configuration
            .iter()
            .find(|entry| entry.key == *configuration_key)
            .and_then(|entry| match &entry.value {
                conduit_core::ConfigurationValue::Structured(value) => (value.profile()
                    == &conduit_language::language_request_profile())
                    .then(|| LanguageRequest::decode(value.canonical_value()).ok())
                    .flatten(),
                _ => None,
            })
            .ok_or_else(|| PlannerError::InvalidLanguageRequest {
                gear_id: gear.gear_id.clone(),
                configuration_key: configuration_key.clone(),
            })?;
        validate_language_request(&request).map_err(|reason| {
            PlannerError::LanguageCoverageUnsatisfied(alloc::boxed::Box::new(
                LanguageCoverageUnsatisfied {
                    gear_id: gear.gear_id.clone(),
                    requirements: vec![LanguageCoverageRequirement {
                        configuration_key: configuration_key.clone(),
                        request: request.clone(),
                    }],
                    candidates: Vec::new(),
                    reason: Some(reason),
                },
            ))
        })?;
        requests.push(LanguageCoverageRequirement {
            configuration_key: configuration_key.clone(),
            request,
        });
    }
    if requests.len() > conduit_core::MAXIMUM_REALIZATION_PROPERTIES {
        return Err(PlannerError::PlannerLimitExceeded(
            "realization requirement bound".into(),
        ));
    }
    Ok(requests)
}

fn checks(
    requests: &[LanguageCoverageRequirement],
    offer: &CapabilityOffer,
) -> Vec<LanguageCoverageCheck> {
    let property = offer
        .realization_properties
        .iter()
        .find(|p| *p.profile() == language_coverage_profile());
    let coverage = property.and_then(|p| LanguageCoverage::decode(p.canonical_value()).ok());
    requests
        .iter()
        .map(|requirement| LanguageCoverageCheck {
            requirement: requirement.clone(),
            coverage_revision: coverage.as_ref().map(|c| c.revision().clone()),
            variety_sensitive: coverage.as_ref().map(|c| *c.variety_sensitive()),
            result: if property.is_some() && coverage.is_none() {
                Err(LanguageCoverageRefusal::MalformedDeclaration)
            } else {
                admit_language_coverage(&requirement.request, coverage.as_ref())
            },
        })
        .collect()
}

pub(crate) fn candidate_checks(
    gear: &CheckedGear,
    offer: &CapabilityOffer,
) -> Result<Vec<LanguageCoverageCheck>, PlannerError> {
    if !conduit_core::valid_realization_properties(&offer.realization_properties) {
        return Err(PlannerError::InvalidRealizationProperties(
            offer.capability_id.clone(),
        ));
    }
    Ok(checks(&requirements(gear)?, offer))
}

fn evidence(
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    checks: Vec<LanguageCoverageCheck>,
) -> LanguageCoverageCandidateEvidence {
    LanguageCoverageCandidateEvidence {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        capability_id: offer.capability_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(),
        checks,
    }
}

pub(crate) fn filter_candidates<T, F>(
    gear: &CheckedGear,
    candidates: Vec<T>,
    fields: F,
) -> Result<Vec<T>, PlannerError>
where
    F: for<'a> Fn(&'a T) -> (&'a HostAdvertisement, &'a CapabilityOffer),
{
    let requests = requirements(gear)?;
    if requests.is_empty() {
        return Ok(candidates);
    }
    if candidates.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(
            "language candidate evidence bound".into(),
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
        let checks = checks(&requests, offer);
        if checks.iter().all(|c| c.result.is_ok()) {
            eligible.push(candidate);
        } else {
            rejected.push(evidence(host, offer, checks));
        }
    }
    if eligible.is_empty() {
        return Err(PlannerError::LanguageCoverageUnsatisfied(
            alloc::boxed::Box::new(LanguageCoverageUnsatisfied {
                gear_id: gear.gear_id.clone(),
                requirements: requests,
                candidates: rejected,
                reason: None,
            }),
        ));
    }
    Ok(eligible)
}

pub(crate) fn validate_selected(
    gear: &CheckedGear,
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
) -> Result<(), PlannerError> {
    filter_candidates(gear, vec![(host, offer)], |candidate| {
        (candidate.0, candidate.1)
    })
    .map(|_| ())
}

/// Realization details over exact current Host/Boot offers; no maker-view policy.
pub fn inspect_language_coverage(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
) -> Result<Vec<LanguageCoverageCandidateEvidence>, PlannerError> {
    let requests = requirements(gear)?;
    let mut records = Vec::new();
    if requests.is_empty() {
        return Ok(records);
    }
    for host in hosts {
        for offer in host
            .capabilities
            .iter()
            .filter(|offer| gear.accepts_realization(offer))
        {
            if records.len() >= MAXIMUM_REALIZATION_DECISION_RECORDS {
                return Err(PlannerError::PlannerLimitExceeded(
                    "language candidate evidence bound".into(),
                ));
            }
            if !conduit_core::valid_realization_properties(&offer.realization_properties) {
                return Err(PlannerError::InvalidRealizationProperties(
                    offer.capability_id.clone(),
                ));
            }
            records.push(evidence(host, offer, checks(&requests, offer)));
        }
    }
    Ok(records)
}

pub(crate) fn no_eligible_error(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
) -> Result<Option<PlannerError>, PlannerError> {
    let requests = requirements(gear)?;
    if requests.is_empty() {
        return Ok(None);
    }
    let candidates = inspect_language_coverage(gear, hosts)?;
    Ok(candidates
        .iter()
        .all(|candidate| candidate.checks.iter().any(|check| check.result.is_err()))
        .then(|| {
            PlannerError::LanguageCoverageUnsatisfied(alloc::boxed::Box::new(
                LanguageCoverageUnsatisfied {
                    gear_id: gear.gear_id.clone(),
                    requirements: requests,
                    candidates,
                    reason: None,
                },
            ))
        }))
}

pub(crate) fn validate_placements(
    gears: &[CheckedGear],
    hosts: &[HostAdvertisement],
    placements: &crate::PlacementChoices,
) -> Result<(), PlannerError> {
    for host in hosts {
        for offer in &host.capabilities {
            if !conduit_core::valid_realization_properties(&offer.realization_properties) {
                return Err(PlannerError::InvalidRealizationProperties(
                    offer.capability_id.clone(),
                ));
            }
        }
    }
    for gear in gears {
        let choice = placements
            .by_gear
            .get(&gear.gear_id)
            .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().into()))?;
        let host = hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().into()))?;
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| PlannerError::UnknownCapability(choice.capability_id.as_str().into()))?;
        super::validate_operation_capability(gear, offer)?;
        validate_selected(gear, host, offer)?;
    }
    Ok(())
}
