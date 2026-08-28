//! Exact std Host offer facts for the reviewed hosted HNSW profile.

use conduit_ai::{
    validate_process_identity, vector_search_contract, vector_search_operation,
    vector_search_startup_parameters, SimilarityMetric, VectorSearchOfferInvalidity,
    VECTOR_SEARCH_RESOURCE_CLASS,
};
use conduit_core::{
    resource_requirement, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer,
};

use super::{
    HostedHnswProfile, HostedHnswProviderIdentity, HostedHnswRefusal, HOSTED_HNSW_ALGORITHM,
    HOSTED_HNSW_IMPLEMENTATION_ID, HOSTED_HNSW_LIBRARY_NAME, HOSTED_HNSW_LIBRARY_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedHnswOfferInvalidity {
    Provider(HostedHnswRefusal),
    Process(VectorSearchOfferInvalidity),
}

pub fn hosted_hnsw_vector_search_offer(
    provider: &HostedHnswProviderIdentity,
    profile: HostedHnswProfile,
) -> Result<CapabilityOffer, HostedHnswOfferInvalidity> {
    provider
        .validate()
        .map_err(HostedHnswOfferInvalidity::Provider)?;
    profile
        .validate()
        .map_err(HostedHnswOfferInvalidity::Provider)?;
    validate_process_identity(&provider.process_identity)
        .map_err(HostedHnswOfferInvalidity::Process)?;

    let contract = vector_search_contract();
    let metric = metric_slug(profile.metric);
    Ok(CapabilityOffer {
        startup_parameters: vector_search_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "vector-search/hnsw/process/{}",
            provider.process_identity
        )),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision.clone(),
        inputs: contract.inputs.clone(),
        outputs: contract.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(format!(
                "conduit.vector-search/{}/{metric}/seed-{}/ef-construction-{}/ef-search-{}@1",
                HOSTED_HNSW_ALGORITHM,
                profile.seed,
                profile.ef_construction,
                profile.ef_search
            )),
            implementation_id: ImplementationId::from(HOSTED_HNSW_IMPLEMENTATION_ID),
            artifact_id: ArtifactId::from(format!(
                "conduit-std-host/vector-search/{HOSTED_HNSW_LIBRARY_NAME}-{HOSTED_HNSW_LIBRARY_VERSION}@1"
            )),
        },
        host_operations: vec![vector_search_operation(&contract)],
        resource_requirements: vec![resource_requirement(
            VECTOR_SEARCH_RESOURCE_CLASS,
            contract.maximum_query_work_units,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    })
}

fn metric_slug(metric: SimilarityMetric) -> &'static str {
    match metric {
        SimilarityMetric::CosineSimilarity => "cosine",
        SimilarityMetric::DotProductSimilarity => "dot-product",
        SimilarityMetric::SquaredEuclideanDistance => "squared-euclidean",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_keeps_every_reviewed_backend_fact_outside_the_portable_face() {
        let provider = HostedHnswProviderIdentity::reviewed("pid-4102").unwrap();
        let profile = HostedHnswProfile {
            metric: SimilarityMetric::CosineSimilarity,
            seed: 7,
            ef_construction: 32,
            ef_search: 16,
        };
        let offer = hosted_hnsw_vector_search_offer(&provider, profile).unwrap();
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            HOSTED_HNSW_IMPLEMENTATION_ID
        );
        assert!(offer
            .implementation
            .artifact_id
            .as_str()
            .contains(HOSTED_HNSW_LIBRARY_NAME));
        assert!(offer
            .implementation
            .artifact_id
            .as_str()
            .contains(HOSTED_HNSW_LIBRARY_VERSION));
        assert!(offer.capability_id.as_str().contains("pid-4102"));
        assert!(offer
            .implementation
            .execution_profile_id
            .as_str()
            .contains("seed-7"));
        assert!(offer
            .implementation
            .execution_profile_id
            .as_str()
            .contains("ef-search-16"));
        assert_eq!(HOSTED_HNSW_ALGORITHM, "hnsw");

        let portable = serde_json::to_string(&vector_search_contract()).unwrap();
        for backend_fact in [
            "hnsw",
            "instant-distance",
            "pid-4102",
            "seed-7",
            "ef-search",
        ] {
            assert!(!portable.contains(backend_fact));
        }
    }
}
