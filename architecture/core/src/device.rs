use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    BaseImplementationId, BaseInstanceId, BootId, CapabilityId, DeviceId, HostAdvertisement,
    HostId, OfferGeneration, ResourceClassId, ResourceHandleId, SignId, PROTOCOL_VERSION,
};

pub const MAXIMUM_DEVICES_PER_HOST: usize = 64;
pub const MAXIMUM_DEVICE_CAPABILITIES: usize = 16;
pub const MAXIMUM_DEVICE_RESOURCES: usize = 16;
pub const MAXIMUM_DEVICE_IDENTITY_FACTS: usize = 16;
pub const MAXIMUM_DEVICE_IDENTITY_FACT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceIdentityStrength {
    /// Identity lasts only for this Boot and exact acquired resource handle.
    BootLocalResource,
    /// The provider asserted an identity with provider-defined continuity.
    ProviderAsserted,
    /// A physical serial was directly observed; uniqueness is still not inferred.
    ObservedPhysicalSerial,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceIdentityFact {
    pub name: alloc::string::String,
    pub value: alloc::string::String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceIdentityEvidence {
    pub strength: DeviceIdentityStrength,
    pub provider: BaseImplementationId,
    pub facts: Vec<DeviceIdentityFact>,
}

/// Inspectable resource provenance. This is neither a resource grant nor a
/// replacement for the Host adapter's authoritative acquired-resource state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceResourceProvenance {
    pub handle_id: ResourceHandleId,
    pub class_id: ResourceClassId,
    pub base_implementation_id: BaseImplementationId,
    pub base_instance_id: BaseInstanceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceTruthDisposition {
    Current,
    /// Retained inspection/replay provenance, never current Host truth.
    HistoricalLost {
        terminal_sign_id: Option<SignId>,
    },
}

/// An optional Host-observed grouping over ordinary capability identities.
/// CapabilityOffer remains the sole operational capability contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAssociation {
    pub protocol_version: u16,
    pub device_id: DeviceId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub disposition: DeviceTruthDisposition,
    pub capability_ids: Vec<CapabilityId>,
    pub resources: Vec<DeviceResourceProvenance>,
    pub identity_evidence: DeviceIdentityEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceAssociationRefusal {
    UnknownVersion,
    TooManyDevices,
    EmptyIdentity,
    InvalidIdentityEvidence,
    InvalidCardinality,
    NonCanonicalOrder,
    DuplicateDevice,
    DuplicateCapability,
    DuplicateResource,
    WrongCurrentHost,
    WrongCurrentBoot,
    WrongCurrentGeneration,
    MissingCurrentCapability,
}

pub fn validate_device_associations(
    advertisement: &HostAdvertisement,
    associations: &[DeviceAssociation],
) -> Result<(), DeviceAssociationRefusal> {
    if associations.len() > MAXIMUM_DEVICES_PER_HOST {
        return Err(DeviceAssociationRefusal::TooManyDevices);
    }
    let mut prior_device = None;
    for association in associations {
        association.validate_shape()?;
        if prior_device.is_some_and(|prior: &DeviceId| prior == &association.device_id) {
            return Err(DeviceAssociationRefusal::DuplicateDevice);
        }
        if prior_device.is_some_and(|prior: &DeviceId| prior > &association.device_id) {
            return Err(DeviceAssociationRefusal::NonCanonicalOrder);
        }
        prior_device = Some(&association.device_id);
        if matches!(association.disposition, DeviceTruthDisposition::Current) {
            if association.host_id != advertisement.host_id {
                return Err(DeviceAssociationRefusal::WrongCurrentHost);
            }
            if association.boot_id != advertisement.boot_id {
                return Err(DeviceAssociationRefusal::WrongCurrentBoot);
            }
            if association.offer_generation != advertisement.offer_generation {
                return Err(DeviceAssociationRefusal::WrongCurrentGeneration);
            }
            for capability_id in &association.capability_ids {
                if !advertisement
                    .capabilities
                    .iter()
                    .any(|offer| &offer.capability_id == capability_id)
                {
                    return Err(DeviceAssociationRefusal::MissingCurrentCapability);
                }
            }
        }
    }
    Ok(())
}

impl DeviceAssociation {
    pub fn validate_shape(&self) -> Result<(), DeviceAssociationRefusal> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(DeviceAssociationRefusal::UnknownVersion);
        }
        if self.device_id.as_str().is_empty()
            || self.host_id.as_str().is_empty()
            || self.boot_id.as_str().is_empty()
            || self.identity_evidence.provider.as_str().is_empty()
        {
            return Err(DeviceAssociationRefusal::EmptyIdentity);
        }
        if self.capability_ids.is_empty()
            || self.capability_ids.len() > MAXIMUM_DEVICE_CAPABILITIES
            || self.resources.len() > MAXIMUM_DEVICE_RESOURCES
            || self.identity_evidence.facts.len() > MAXIMUM_DEVICE_IDENTITY_FACTS
        {
            return Err(DeviceAssociationRefusal::InvalidCardinality);
        }
        if self.identity_evidence.facts.iter().any(|fact| {
            fact.name.is_empty()
                || fact.value.is_empty()
                || fact.name.len() > MAXIMUM_DEVICE_IDENTITY_FACT_BYTES
                || fact.value.len() > MAXIMUM_DEVICE_IDENTITY_FACT_BYTES
        }) {
            return Err(DeviceAssociationRefusal::InvalidIdentityEvidence);
        }
        match self.identity_evidence.strength {
            DeviceIdentityStrength::BootLocalResource if self.resources.is_empty() => {
                return Err(DeviceAssociationRefusal::InvalidIdentityEvidence);
            }
            DeviceIdentityStrength::ProviderAsserted if self.identity_evidence.facts.is_empty() => {
                return Err(DeviceAssociationRefusal::InvalidIdentityEvidence);
            }
            DeviceIdentityStrength::ObservedPhysicalSerial
                if !self
                    .identity_evidence
                    .facts
                    .iter()
                    .any(|fact| fact.name == "physical-serial") =>
            {
                return Err(DeviceAssociationRefusal::InvalidIdentityEvidence);
            }
            _ => {}
        }
        canonical_unique(
            &self.capability_ids,
            DeviceAssociationRefusal::DuplicateCapability,
        )?;
        canonical_unique(&self.resources, DeviceAssociationRefusal::DuplicateResource)?;
        canonical_unique(
            &self.identity_evidence.facts,
            DeviceAssociationRefusal::InvalidIdentityEvidence,
        )?;
        Ok(())
    }
}

fn canonical_unique<T: Ord>(
    values: &[T],
    duplicate: DeviceAssociationRefusal,
) -> Result<(), DeviceAssociationRefusal> {
    if values.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(DeviceAssociationRefusal::NonCanonicalOrder);
    }
    let unique = values.iter().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(duplicate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::{
        ArtifactId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostProfileId,
        ImplementationId, ImplementationOffer, KindContractRevision, KindId,
        PlannerCapabilityOffer,
    };

    fn offer(id: &str) -> CapabilityOffer {
        CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from(id),
            kind_id: KindId::from("input/example@1"),
            kind_contract_revision: KindContractRevision::from("revision/1"),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("fixture/profile@1"),
                implementation_id: ImplementationId::from("fixture/input@1"),
                artifact_id: ArtifactId::from("fixture/artifact@1"),
            },
            inputs: Vec::new(),
            outputs: Vec::new(),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }
    }

    fn advertisement() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("host/current"),
            boot_id: BootId::from("boot/current"),
            offer_generation: OfferGeneration(7),
            profile: HostProfileId::from("fixture/profile@1"),
            resources: Vec::new(),
            capabilities: vec![offer("button"), offer("haptic")],
            planner_capabilities: Vec::<PlannerCapabilityOffer>::new(),
        }
    }

    fn association(disposition: DeviceTruthDisposition) -> DeviceAssociation {
        DeviceAssociation {
            protocol_version: PROTOCOL_VERSION,
            device_id: DeviceId::from("device/controller"),
            host_id: HostId::from("host/current"),
            boot_id: BootId::from("boot/current"),
            offer_generation: OfferGeneration(7),
            disposition,
            capability_ids: vec![CapabilityId::from("button"), CapabilityId::from("haptic")],
            resources: vec![DeviceResourceProvenance {
                handle_id: ResourceHandleId::from("resource/one"),
                class_id: ResourceClassId::from("resource/controller@1"),
                base_implementation_id: BaseImplementationId::from("fixture/controller@1"),
                base_instance_id: BaseInstanceId::from("fixture/controller/one"),
            }],
            identity_evidence: DeviceIdentityEvidence {
                strength: DeviceIdentityStrength::BootLocalResource,
                provider: BaseImplementationId::from("fixture/controller@1"),
                facts: vec![DeviceIdentityFact {
                    name: "scope".into(),
                    value: "current-boot-resource".into(),
                }],
            },
        }
    }

    #[test]
    fn optional_and_composite_current_associations_validate() {
        let advertisement = advertisement();
        assert_eq!(validate_device_associations(&advertisement, &[]), Ok(()));
        assert_eq!(
            validate_device_associations(
                &advertisement,
                &[association(DeviceTruthDisposition::Current)]
            ),
            Ok(())
        );
    }

    #[test]
    fn stale_current_truth_and_dangling_capabilities_refuse() {
        let advertisement = advertisement();
        let mut stale = association(DeviceTruthDisposition::Current);
        stale.boot_id = BootId::from("boot/old");
        assert_eq!(
            validate_device_associations(&advertisement, &[stale]),
            Err(DeviceAssociationRefusal::WrongCurrentBoot)
        );
        let mut dangling = association(DeviceTruthDisposition::Current);
        dangling.capability_ids = vec![CapabilityId::from("missing")];
        assert_eq!(
            validate_device_associations(&advertisement, &[dangling]),
            Err(DeviceAssociationRefusal::MissingCurrentCapability)
        );
    }

    #[test]
    fn old_truth_is_retained_only_when_explicitly_historical() {
        let advertisement = advertisement();
        let mut historical = association(DeviceTruthDisposition::HistoricalLost {
            terminal_sign_id: Some(SignId::from("sign/lost")),
        });
        historical.boot_id = BootId::from("boot/old");
        historical.offer_generation = OfferGeneration(3);
        assert_eq!(
            validate_device_associations(&advertisement, &[historical]),
            Ok(())
        );
    }

    #[test]
    fn vendor_product_facts_cannot_masquerade_as_physical_identity() {
        let mut weak = association(DeviceTruthDisposition::Current);
        weak.identity_evidence.strength = DeviceIdentityStrength::ObservedPhysicalSerial;
        weak.identity_evidence.facts = vec![
            DeviceIdentityFact {
                name: "usb-product-id".into(),
                value: "000a".into(),
            },
            DeviceIdentityFact {
                name: "usb-vendor-id".into(),
                value: "2e8a".into(),
            },
        ];
        assert_eq!(
            validate_device_associations(&advertisement(), &[weak]),
            Err(DeviceAssociationRefusal::InvalidIdentityEvidence)
        );
    }
}
