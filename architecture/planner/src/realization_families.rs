use crate::prelude::*;
use crate::{CandidateStructure, PlannerError};
use alloc::collections::{BTreeMap, BTreeSet};

pub const MAXIMUM_REALIZATION_FAMILIES: usize = 32;
pub const MAXIMUM_CURRENT_FAMILY_OFFERS: usize = 32;
pub const MAXIMUM_REALIZATION_FAMILY_PREREQUISITES: usize = 16;

/// Durable semantic/implementation knowledge. This deliberately contains no
/// Host, Boot, Offer, Resource, Authority, Line, benchmark, or Plan snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationFamily {
    pub family_id: String,
    pub semantic_contract_id: String,
    pub implementation_contract_id: String,
    pub implementation_contract_revision: u64,
    pub prerequisite_contract_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationFamilyCatalog {
    families: Vec<RealizationFamily>,
}

impl RealizationFamilyCatalog {
    pub fn new(families: Vec<RealizationFamily>) -> Result<Self, PlannerError> {
        if families.is_empty() || families.len() > MAXIMUM_REALIZATION_FAMILIES {
            return invalid("realization family count is empty or exceeds its finite bound");
        }
        let mut family_ids = BTreeSet::new();
        for family in &families {
            let prerequisites = family
                .prerequisite_contract_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if family.family_id.is_empty()
                || family.semantic_contract_id.is_empty()
                || family.implementation_contract_id.is_empty()
                || family.implementation_contract_revision == 0
                || !family_ids.insert(family.family_id.as_str())
                || family.prerequisite_contract_ids.is_empty()
                || family.prerequisite_contract_ids.len() > MAXIMUM_REALIZATION_FAMILY_PREREQUISITES
                || prerequisites.len() != family.prerequisite_contract_ids.len()
                || prerequisites.iter().any(|identity| identity.is_empty())
            {
                return invalid("realization family knowledge is malformed or ambiguous");
            }
        }
        Ok(Self { families })
    }

    pub fn families(&self) -> &[RealizationFamily] {
        &self.families
    }

    pub fn family(&self, family_id: &str) -> Option<&RealizationFamily> {
        self.families
            .iter()
            .find(|family| family.family_id == family_id)
    }
}

/// A fresh planning-time offer. `policy_rank` is current reviewed policy/cost
/// truth supplied by the ordinary planner, never retained in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentFamilyOffer {
    pub family_id: String,
    pub semantic_contract_id: String,
    pub implementation_contract_id: String,
    pub implementation_contract_revision: u64,
    pub satisfied_prerequisite_contract_ids: Vec<String>,
    pub policy_rank: u64,
    pub candidate: CandidateStructure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyFrontierMetrics {
    pub cataloged_families: u32,
    pub current_offers: u32,
    pub frontier_candidates: u32,
    pub dominated_candidates_not_explored: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyFrontier {
    pub candidates: Vec<CandidateStructure>,
    pub metrics: FamilyFrontierMetrics,
}

/// Returns only the best current policy rank. Dominated families remain in the
/// independent catalog and therefore become eligible on a later fresh call if
/// preferred prerequisites disappear.
pub fn select_current_family_frontier(
    catalog: &RealizationFamilyCatalog,
    offers: &[CurrentFamilyOffer],
) -> Result<FamilyFrontier, PlannerError> {
    if offers.is_empty() || offers.len() > MAXIMUM_CURRENT_FAMILY_OFFERS {
        return invalid("current family offer count is empty or exceeds its finite bound");
    }
    let catalog_by_id = catalog
        .families
        .iter()
        .map(|family| (family.family_id.as_str(), family))
        .collect::<BTreeMap<_, _>>();
    let mut offered_family_ids = BTreeSet::new();
    for offer in offers {
        let family = catalog_by_id.get(offer.family_id.as_str()).ok_or_else(|| {
            PlannerError::InvalidRealizationPolicy(
                "current offer names an uncataloged realization family".to_string(),
            )
        })?;
        let satisfied = offer
            .satisfied_prerequisite_contract_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required = family
            .prerequisite_contract_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if !offered_family_ids.insert(offer.family_id.as_str())
            || offer.semantic_contract_id != family.semantic_contract_id
            || offer.implementation_contract_id != family.implementation_contract_id
            || offer.implementation_contract_revision != family.implementation_contract_revision
            || offer.candidate.semantic_contract_id != family.semantic_contract_id
            || offer.candidate.implementation_family_id != family.implementation_contract_id
            || satisfied != required
        {
            return invalid(
                "current family offer is duplicate, incomplete, or contract-incompatible",
            );
        }
    }

    let best_rank = offers
        .iter()
        .map(|offer| offer.policy_rank)
        .min()
        .expect("nonempty current offers were validated");
    let candidates = offers
        .iter()
        .filter(|offer| offer.policy_rank == best_rank)
        .map(|offer| offer.candidate.clone())
        .collect::<Vec<_>>();
    Ok(FamilyFrontier {
        metrics: FamilyFrontierMetrics {
            cataloged_families: count(catalog.families.len()),
            current_offers: count(offers.len()),
            frontier_candidates: count(candidates.len()),
            dominated_candidates_not_explored: count(offers.len() - candidates.len()),
        },
        candidates,
    })
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(detail.to_string()))
}
