use crate::{CheckedCanonicalPlot, CheckedGear};

impl CheckedGear {
    pub fn checked_front(&self) -> conduit_core::CheckedFront {
        conduit_core::CheckedFront::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
        .with_value_contracts(self.semantic_contract.value_contracts().to_vec())
    }

    /// Tests both callable fit and semantic realization eligibility.
    ///
    /// Ordinary gears require the same semantic contract identity. A semantic
    /// owner may deliberately request structural polymorphism with the
    /// reviewed marker; that is an authored meaning, not a planner fallback.
    /// Structural polymorphism relaxes exact Kind identity, never the Kind ID:
    /// a merely front-compatible offer for another Kind is not a Back for this
    /// Gear.
    pub fn accepts_realization(&self, offer: &conduit_core::CapabilityOffer) -> bool {
        self.kind_id == offer.kind_id
            && self.checked_front() == offer.checked_front()
            && self.accepts_semantic_contract(offer)
            && (self.kind_contract_revision == offer.kind_contract_revision
                || self.kind_contract_revision.as_str()
                    == conduit_core::STRUCTURAL_POLYMORPHIC_CONTRACT)
    }

    /// Exact Kind law and configuration-schema equality, excluding the finite
    /// checker placeholder carried by a mandatory startup field. Mandatory
    /// authored values belong to this Gear's `configuration`; only actual Kind
    /// defaults (`has_default = true`) are part of realization identity.
    pub fn accepts_semantic_contract(&self, offer: &conduit_core::CapabilityOffer) -> bool {
        self.semantic_contract.laws == offer.semantic_contract.laws
            && self.semantic_contract.configuration.len()
                == offer.semantic_contract.configuration.len()
            && self
                .semantic_contract
                .configuration
                .iter()
                .zip(&offer.semantic_contract.configuration)
                .all(|(gear, offered)| {
                    gear.key == offered.key
                        && gear.rule == offered.rule
                        && self
                            .startup_parameters
                            .iter()
                            .find(|parameter| parameter.name == gear.key)
                            .is_some_and(|parameter| {
                                !parameter.has_default
                                    || gear.default_value == offered.default_value
                            })
                })
    }

    #[doc(hidden)]
    pub fn validate_constructed_semantic_contract(
        &self,
    ) -> Result<(), conduit_core::KindValidationError> {
        conduit_core::Kind {
            startup_parameters: self.startup_parameters.clone(),
            shorthand: self.shorthand.clone(),
            kind_id: self.kind_id.clone(),
            kind_contract_revision: self.kind_contract_revision.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            configuration: self.semantic_contract.configuration.clone(),
            semantic_laws: self.semantic_contract.laws.clone(),
            limits: conduit_core::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }
        .validate()
    }
}

/// Reviewed named-field construction for checked Plot Gears.
#[macro_export]
macro_rules! checked_gear_from_parts {
    ($($fields:tt)*) => {{
        let gear = $crate::CheckedGear { $($fields)* };
        if let Err(error) = gear.validate_constructed_semantic_contract() {
            panic!(
                "CheckedGear '{}' ({}) requires a valid semantic contract: {:?}",
                gear.kind_id.as_str(),
                gear.kind_contract_revision.as_str(),
                error
            );
        }
        gear
    }};
}

impl CheckedCanonicalPlot {
    pub fn checked_front(&self) -> conduit_core::CheckedFront {
        self.runtime_front.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use conduit_core::{
        kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits,
        ExecutionProfileId, ExternalEffectBehavior, ImplementationId, Kind, KindIdentity,
        KindSemanticLaw,
    };

    fn kind(effect: ExternalEffectBehavior) -> Kind {
        Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("test/exact-contract"),
            kind_contract_revision: KindIdentity::from("contract-v1"),
            inputs: Vec::new(),
            outputs: Vec::new(),
            configuration: Vec::new(),
            semantic_laws: vec![KindSemanticLaw::ExternalEffects(effect)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }
    }

    fn offer(kind: Kind) -> conduit_core::CapabilityOffer {
        BackOfferBuilder::new(
            kind,
            Back {
                capability_id: CapabilityId::from("capability"),
                execution_profile_id: ExecutionProfileId::from("native"),
                implementation_id: ImplementationId::from("implementation"),
                artifact_id: ArtifactId::from("artifact"),
                host_calls: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
            },
        )
        .build()
    }

    fn checked_gear(kind: &Kind) -> CheckedGear {
        crate::checked_gear_from_parts! {
            gear_id: conduit_core::GearId::from("gear"),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            semantic_contract: kind.semantic_contract(),
            terminal_transductions: Vec::new(),
            resource_ports: Vec::new(),
            configuration: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    #[test]
    fn eligibility_refuses_same_identity_and_front_with_different_semantic_law() {
        let expected = kind(ExternalEffectBehavior::None);
        let actual = kind(ExternalEffectBehavior::Observable);

        assert!(!checked_gear(&expected).accepts_realization(&offer(actual)));
    }

    #[test]
    fn equivalent_alternate_back_retains_contract_and_distinct_implementation() {
        let contract = kind(ExternalEffectBehavior::None);
        let first = offer(contract.clone());
        let mut alternate = offer(contract.clone());
        alternate.implementation.implementation_id = ImplementationId::from("alternate");

        let gear = checked_gear(&contract);
        assert!(gear.accepts_realization(&first));
        assert!(gear.accepts_realization(&alternate));
        assert_eq!(first.semantic_contract, alternate.semantic_contract);
        assert_ne!(
            first.implementation.implementation_id,
            alternate.implementation.implementation_id
        );
    }
}
