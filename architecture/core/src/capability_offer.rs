use crate::{
    ArtifactId, AuthorityRequirement, CapabilityId, CapabilityLimits, CapabilityOffer,
    CheckedFront, ExecutionProfileId, FrontStartupParameter, HostCallRequirement, ImplementationId,
    ImplementationOffer, KindConfigurationField, KindId, KindIdentity, KindSemanticLaw,
    PortDescriptor, PortId, ResourceRequirement,
};
use alloc::{collections::BTreeSet, vec::Vec};

/// Portable semantic truth from which a host may offer one realization.
///
/// This deliberately contains no implementation, artifact, Host Call,
/// resource, or authority identity. Those belong to the realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kind {
    pub startup_parameters: Vec<FrontStartupParameter>,
    pub shorthand: Option<(PortId, PortId)>,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<KindConfigurationField>,
    pub semantic_laws: Vec<KindSemanticLaw>,
    pub limits: CapabilityLimits,
}

impl Kind {
    pub fn checked_front(&self) -> CheckedFront {
        CheckedFront::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
    }

    pub fn validate(&self) -> Result<(), KindValidationError> {
        if self.kind_id.as_str().is_empty() {
            return Err(KindValidationError::EmptyId);
        }
        if self.kind_contract_revision.as_str().is_empty() {
            return Err(KindValidationError::EmptyIdentity);
        }
        let mut keys = BTreeSet::new();
        for field in &self.configuration {
            if !keys.insert(field.key.as_str()) {
                return Err(KindValidationError::DuplicateConfigurationKey);
            }
            let Some(front) = self
                .startup_parameters
                .iter()
                .find(|parameter| parameter.name == field.key)
            else {
                return Err(KindValidationError::ConfigurationMissingFromFront);
            };
            // Configuration owns the canonical value and rule. The callable
            // Front independently owns whether authors may omit it.
            if front.value_type != field.default_value.semantic_kind() {
                return Err(KindValidationError::ConfigurationFrontMismatch);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindValidationError {
    EmptyId,
    EmptyIdentity,
    DuplicateConfigurationKey,
    ConfigurationMissingFromFront,
    ConfigurationFrontMismatch,
}

/// Host-owned identity and requirements for one semantic realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Back {
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub host_calls: Vec<HostCallRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
}

/// A realization attempted to advertise more capacity than its semantic
/// contract admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityCapacityError {
    ActiveInstances,
    QueueItems,
    QueueBytes,
}

/// Canonical constructor for a host capability offer.
///
/// Semantic fields are supplied once by the checked contract. A realization
/// may keep those limits or explicitly narrow them, but cannot broaden them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackOfferBuilder {
    contract: Kind,
    realization: Back,
    realization_limits: CapabilityLimits,
}

impl BackOfferBuilder {
    pub fn new(contract: Kind, realization: Back) -> Self {
        if let Err(error) = contract.validate() {
            panic!(
                "Back offers require a valid canonical Kind '{}': {error:?}",
                contract.kind_id.as_str()
            );
        }
        let realization_limits = contract.limits.clone();
        Self {
            contract,
            realization,
            realization_limits,
        }
    }

    pub fn try_new(contract: Kind, realization: Back) -> Result<Self, KindValidationError> {
        contract.validate()?;
        let realization_limits = contract.limits.clone();
        Ok(Self {
            contract,
            realization,
            realization_limits,
        })
    }

    pub fn narrow_capacity(
        mut self,
        limits: CapabilityLimits,
    ) -> Result<Self, CapabilityCapacityError> {
        if limits.max_active_instances > self.contract.limits.max_active_instances {
            return Err(CapabilityCapacityError::ActiveInstances);
        }
        if limits.max_queue_items > self.contract.limits.max_queue_items {
            return Err(CapabilityCapacityError::QueueItems);
        }
        if limits.max_queue_bytes > self.contract.limits.max_queue_bytes {
            return Err(CapabilityCapacityError::QueueBytes);
        }
        self.realization_limits = limits;
        Ok(self)
    }

    pub fn build(self) -> CapabilityOffer {
        CapabilityOffer {
            startup_parameters: self.contract.startup_parameters,
            shorthand: self.contract.shorthand,
            capability_id: self.realization.capability_id,
            kind_id: self.contract.kind_id,
            kind_contract_revision: self.contract.kind_contract_revision,
            inputs: self.contract.inputs,
            outputs: self.contract.outputs,
            implementation: ImplementationOffer {
                execution_profile_id: self.realization.execution_profile_id,
                implementation_id: self.realization.implementation_id,
                artifact_id: self.realization.artifact_id,
            },
            host_calls: self.realization.host_calls,
            resource_requirements: self.realization.resource_requirements,
            authority_requirements: self.realization.authority_requirements,
            limits: self.realization_limits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, port_id, PortDirection, PortTemporal};
    use alloc::vec;

    fn contract() -> Kind {
        Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("test/semantic"),
            kind_contract_revision: KindIdentity::from("contract-v1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                direction: PortDirection::Input,
                value_kind: kind_id("value/count"),
                temporal: PortTemporal::Value,
            }],
            outputs: Vec::new(),
            configuration: Default::default(),
            semantic_laws: Default::default(),
            limits: CapabilityLimits {
                max_active_instances: 4,
                max_queue_items: 8,
                max_queue_bytes: 64,
            },
        }
    }

    fn realization(implementation: &str) -> Back {
        Back {
            capability_id: CapabilityId::from("host-capability"),
            execution_profile_id: ExecutionProfileId::from("native"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("artifact"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        }
    }

    #[test]
    fn construction_keeps_semantics_owned_by_the_contract() {
        let expected = contract();
        let offer = BackOfferBuilder::new(expected.clone(), realization("impl-a")).build();
        assert_eq!(offer.kind_id, expected.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            expected.kind_contract_revision
        );
        assert_eq!(offer.inputs, expected.inputs);
        assert_eq!(offer.outputs, expected.outputs);
        assert_eq!(offer.limits, expected.limits);

        let other = BackOfferBuilder::new(expected, realization("impl-b")).build();
        assert_eq!(offer.kind_id, other.kind_id);
        assert_eq!(offer.kind_contract_revision, other.kind_contract_revision);
        assert_eq!(offer.inputs, other.inputs);
        assert_ne!(offer.implementation, other.implementation);
    }

    #[test]
    fn realization_capacity_may_only_narrow() {
        let narrowed = CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 3,
            max_queue_bytes: 32,
        };
        let offer = BackOfferBuilder::new(contract(), realization("impl"))
            .narrow_capacity(narrowed.clone())
            .unwrap()
            .build();
        assert_eq!(offer.limits, narrowed);

        for (limits, expected) in [
            (
                CapabilityLimits {
                    max_active_instances: 5,
                    max_queue_items: 8,
                    max_queue_bytes: 64,
                },
                CapabilityCapacityError::ActiveInstances,
            ),
            (
                CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 9,
                    max_queue_bytes: 64,
                },
                CapabilityCapacityError::QueueItems,
            ),
            (
                CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 8,
                    max_queue_bytes: 65,
                },
                CapabilityCapacityError::QueueBytes,
            ),
        ] {
            assert_eq!(
                BackOfferBuilder::new(contract(), realization("impl"))
                    .narrow_capacity(limits)
                    .unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn kind_validation_refuses_duplicate_or_front_mismatched_configuration() {
        let mut kind = contract();
        kind.startup_parameters = vec![FrontStartupParameter {
            name: "count".into(),
            value_type: kind_id(crate::COUNT_INFO_ID),
            has_default: true,
        }];
        kind.configuration = vec![KindConfigurationField {
            key: "count".into(),
            default_value: crate::ConfigurationValue::U64(1),
            rule: crate::KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 8,
            },
        }];
        assert_eq!(kind.validate(), Ok(()));

        kind.startup_parameters[0].has_default = false;
        assert_eq!(kind.validate(), Ok(()));

        kind.configuration.push(kind.configuration[0].clone());
        assert_eq!(
            kind.validate(),
            Err(KindValidationError::DuplicateConfigurationKey)
        );
        assert_eq!(
            BackOfferBuilder::try_new(kind.clone(), realization("invalid")).unwrap_err(),
            KindValidationError::DuplicateConfigurationKey
        );
        kind.configuration.pop();
        kind.startup_parameters[0].value_type = kind_id(crate::TEXT_INFO_ID);
        assert_eq!(
            kind.validate(),
            Err(KindValidationError::ConfigurationFrontMismatch)
        );
    }
}
