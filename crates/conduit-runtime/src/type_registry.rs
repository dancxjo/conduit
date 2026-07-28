//! Hosted discovery and compatibility for domain-owned type contracts.

use std::collections::BTreeMap;
use std::fmt;

use conduit_core::{
    CanonicalDescriptor, CompatibilityClass, CompatibilityDecision, CompatibilityOutcome,
    CompatibilityQuery, CompatibilityReason, FlowPolicy, FlowPolicyDecision, FlowTypeFacts, Id,
    SemanticHash, TraitProof, TypeContractRef,
};

/// Comparison behavior declared by an exact type-contract descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeComparisonStrategy<'a> {
    /// Exact identity is sufficient; non-exact revisions require a provider rule.
    Nominal,
    /// Both contracts must opt into comparison of this canonical projection.
    Structural {
        /// Semantic identity of the provider-defined structural projection.
        shape: SemanticHash,
    },
    /// Only the owning domain provider may interpret compatibility.
    Opaque,
    /// A future or malformed strategy this registry cannot execute.
    Unknown(Id<'a>),
}

/// Provider-owned discovery result for one exact type contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeContractDescription<'a> {
    /// Human-readable, non-identity label.
    pub human_name: &'a str,
    /// Exact immutable descriptor whose hash is carried by the reference.
    pub descriptor: CanonicalDescriptor<'a>,
    /// Comparison strategy interpreted from that exact descriptor.
    pub strategy: TypeComparisonStrategy<'a>,
    /// Type-owned facts used by exact bounded-flow resolution.
    pub flow_type_facts: FlowTypeFacts<'a>,
}

/// A provider's reasoned answer before the registry adds exact operands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderTypeDecision<'a> {
    /// Proven, disproven, or dependent on another named fact.
    pub outcome: CompatibilityOutcome,
    /// Stable provider-owned rule or missing-fact identifier.
    pub rule: Id<'a>,
}

/// A namespace-scoped source of type descriptors and compatibility rules.
pub trait TypeContractProvider {
    /// Namespace selected from the contract identifier before `/`.
    fn namespace(&self) -> &str;

    /// Discovers one exact immutable descriptor and its human-facing name.
    fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>>;

    /// Answers whether the consumer accepts every value from the producer.
    fn consumer_accepts_producer<'a>(
        &'a self,
        consumer: TypeContractRef<'a>,
        producer: TypeContractRef<'a>,
    ) -> ProviderTypeDecision<'a>;
}

/// Provider registration failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRegistryError {
    /// A provider namespace is not one portable local identifier.
    InvalidNamespace(String),
    /// A provider is already registered for the namespace.
    DuplicateNamespace(String),
}

impl fmt::Display for TypeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespace(namespace) => {
                write!(formatter, "invalid type provider namespace `{namespace}`")
            }
            Self::DuplicateNamespace(namespace) => {
                write!(formatter, "duplicate type provider namespace `{namespace}`")
            }
        }
    }
}

impl std::error::Error for TypeRegistryError {}

/// Deterministically ordered hosted registry of domain type providers.
#[derive(Default)]
pub struct TypeRegistry {
    providers: BTreeMap<String, Box<dyn TypeContractProvider>>,
}

impl TypeRegistry {
    /// Registers the only provider for its exact namespace.
    pub fn register<P>(&mut self, provider: P) -> Result<(), TypeRegistryError>
    where
        P: TypeContractProvider + 'static,
    {
        let namespace = provider.namespace();
        if Id::new(namespace).is_err() || namespace.contains('/') {
            return Err(TypeRegistryError::InvalidNamespace(namespace.to_owned()));
        }
        if self.providers.contains_key(namespace) {
            return Err(TypeRegistryError::DuplicateNamespace(namespace.to_owned()));
        }
        self.providers
            .insert(namespace.to_owned(), Box::new(provider));
        Ok(())
    }

    /// Returns a verified provider description, if one is available.
    #[must_use]
    pub fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>> {
        if reference.validate().is_err() {
            return None;
        }
        let provider = self.provider(reference)?;
        let description = provider.describe(reference)?;
        description_matches(reference, &description).then_some(description)
    }

    /// Answers the exact directional type question with stable operands.
    #[must_use]
    pub fn consumer_accepts_producer<'a>(
        &'a self,
        consumer: TypeContractRef<'a>,
        producer: TypeContractRef<'a>,
    ) -> CompatibilityDecision<'a> {
        let query = CompatibilityQuery::ConsumerAcceptsProducer { consumer, producer };

        for reference in [consumer, producer] {
            if reference.validate().is_err() {
                return CompatibilityDecision::indeterminate(
                    query,
                    CompatibilityReason::InvalidTypeReference,
                    Some(reference.contract_id),
                );
            }
        }

        if consumer == producer {
            return CompatibilityDecision::compatible(
                query,
                CompatibilityClass::Exact,
                CompatibilityReason::TypeContractExact,
                None,
            );
        }

        let Some(consumer_provider) = self.provider(consumer) else {
            return CompatibilityDecision::indeterminate(
                query,
                CompatibilityReason::TypeProviderUnavailable,
                consumer.namespace().ok(),
            );
        };
        let Some(producer_provider) = self.provider(producer) else {
            return CompatibilityDecision::indeterminate(
                query,
                CompatibilityReason::TypeProviderUnavailable,
                producer.namespace().ok(),
            );
        };
        let Some(consumer_description) = consumer_provider.describe(consumer) else {
            return CompatibilityDecision::indeterminate(
                query,
                CompatibilityReason::TypeContractUnknown,
                Some(consumer.contract_id),
            );
        };
        let Some(producer_description) = producer_provider.describe(producer) else {
            return CompatibilityDecision::indeterminate(
                query,
                CompatibilityReason::TypeContractUnknown,
                Some(producer.contract_id),
            );
        };
        for (reference, description) in [
            (consumer, &consumer_description),
            (producer, &producer_description),
        ] {
            if !description_matches(reference, description) {
                return CompatibilityDecision::indeterminate(
                    query,
                    CompatibilityReason::TypeDescriptorInvalid,
                    Some(reference.contract_id),
                );
            }
        }

        match (consumer_description.strategy, producer_description.strategy) {
            (TypeComparisonStrategy::Unknown(strategy), _)
            | (_, TypeComparisonStrategy::Unknown(strategy)) => {
                CompatibilityDecision::indeterminate(
                    query,
                    CompatibilityReason::TypeStrategyUnknown,
                    Some(strategy),
                )
            }
            (
                TypeComparisonStrategy::Structural {
                    shape: consumer_shape,
                },
                TypeComparisonStrategy::Structural {
                    shape: producer_shape,
                },
            ) if consumer_shape == producer_shape => CompatibilityDecision::compatible(
                query,
                CompatibilityClass::Accepted,
                CompatibilityReason::TypeStructuralAccepted,
                None,
            ),
            (
                TypeComparisonStrategy::Structural { .. },
                TypeComparisonStrategy::Structural { .. },
            ) => CompatibilityDecision::incompatible(
                query,
                CompatibilityReason::TypeStructuralMismatch,
                None,
            ),
            (TypeComparisonStrategy::Structural { .. }, _)
            | (_, TypeComparisonStrategy::Structural { .. }) => {
                CompatibilityDecision::incompatible(
                    query,
                    CompatibilityReason::TypeStrategyMismatch,
                    None,
                )
            }
            (TypeComparisonStrategy::Nominal, TypeComparisonStrategy::Nominal)
            | (TypeComparisonStrategy::Opaque, TypeComparisonStrategy::Opaque) => {
                provider_decision(
                    query,
                    consumer_provider.consumer_accepts_producer(consumer, producer),
                )
            }
            _ => CompatibilityDecision::incompatible(
                query,
                CompatibilityReason::TypeStrategyMismatch,
                None,
            ),
        }
    }

    /// Assesses one exact flow policy against provider-owned type facts.
    #[must_use]
    pub fn assess_flow_policy(
        &self,
        reference: TypeContractRef<'_>,
        policy: FlowPolicy<'_>,
    ) -> FlowPolicyDecision {
        let facts = self
            .describe(reference)
            .map(|description| description.flow_type_facts)
            .unwrap_or(FlowTypeFacts {
                disposable: TraitProof::Indeterminate,
                coalescers: None,
            });
        policy.assess_type_facts(facts)
    }

    fn provider(&self, reference: TypeContractRef<'_>) -> Option<&dyn TypeContractProvider> {
        let namespace = reference.namespace().ok()?;
        self.providers.get(namespace.as_str()).map(Box::as_ref)
    }
}

fn description_matches(
    reference: TypeContractRef<'_>,
    description: &TypeContractDescription<'_>,
) -> bool {
    description.descriptor.kind == reference.contract_id
        && description.descriptor.schema_version == reference.schema_version
        && description.descriptor.semantic_hash().ok() == Some(reference.semantic_hash)
}

fn provider_decision<'a>(
    query: CompatibilityQuery<'a>,
    provider: ProviderTypeDecision<'a>,
) -> CompatibilityDecision<'a> {
    if Id::new(provider.rule.as_str()).is_err() {
        return CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::TypeProviderDecisionInvalid,
            Some(provider.rule),
        );
    }
    match provider.outcome {
        CompatibilityOutcome::Compatible => CompatibilityDecision::compatible(
            query,
            CompatibilityClass::Accepted,
            CompatibilityReason::TypeProviderAccepted,
            Some(provider.rule),
        ),
        CompatibilityOutcome::Incompatible => CompatibilityDecision::incompatible(
            query,
            CompatibilityReason::TypeProviderRejected,
            Some(provider.rule),
        ),
        CompatibilityOutcome::Indeterminate => CompatibilityDecision::indeterminate(
            query,
            CompatibilityReason::TypeProviderIndeterminate,
            Some(provider.rule),
        ),
    }
}
