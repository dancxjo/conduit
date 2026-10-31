//! Complete immutable planning descriptions; possession stays with actual owners.
use crate::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceRequirementRef<'a> {
    pub class_id: &'a str,
    pub units: u32,
    pub protected_role: Option<&'a str>,
    pub compute: Option<&'a ComputeRequirement>,
    pub content: Option<&'a ResourceContentRequirement>,
}
impl<'a> From<&'a ResourceRequirement> for ResourceRequirementRef<'a> {
    fn from(v: &'a ResourceRequirement) -> Self {
        Self {
            class_id: v.class_id.as_str(),
            units: v.units,
            protected_role: v.protected_role.as_ref().map(|id| id.as_str()),
            compute: v.compute.as_ref(),
            content: v.content.as_ref(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceOfferRef<'a> {
    pub pool_id: &'a str,
    pub class_id: &'a str,
    pub capacity_units: u32,
    pub compute: Option<&'a ComputePoolContract>,
    pub content: Option<&'a ResourceContentOffer>,
}
impl<'a> From<&'a ResourceOffer> for ResourceOfferRef<'a> {
    fn from(v: &'a ResourceOffer) -> Self {
        Self {
            pool_id: v.pool_id.as_str(),
            class_id: v.class_id.as_str(),
            capacity_units: v.capacity_units,
            compute: v.compute.as_ref(),
            content: v.content.as_ref(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuthorityRequirementRef<'a> {
    pub contract_id: &'a str,
    pub host_call_contract_id: &'a str,
    pub subject_kind: &'a str,
}
impl<'a> From<&'a AuthorityRequirement> for AuthorityRequirementRef<'a> {
    fn from(v: &'a AuthorityRequirement) -> Self {
        Self {
            contract_id: v.contract_id.as_str(),
            host_call_contract_id: v.host_call_contract_id.as_str(),
            subject_kind: v.subject_kind.as_str(),
        }
    }
}
#[derive(Clone, Copy)]
pub enum ResourceRequirementRefs<'a> {
    Owned(&'a [ResourceRequirement]),
    Borrowed(&'a [ResourceRequirementRef<'a>]),
}
impl<'a> ResourceRequirementRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = ResourceRequirementRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub enum ResourceOfferRefs<'a> {
    Owned(&'a [ResourceOffer]),
    Borrowed(&'a [ResourceOfferRef<'a>]),
}
impl<'a> ResourceOfferRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = ResourceOfferRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub enum AuthorityRequirementRefs<'a> {
    Owned(&'a [AuthorityRequirement]),
    Borrowed(&'a [AuthorityRequirementRef<'a>]),
}
impl<'a> AuthorityRequirementRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = AuthorityRequirementRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub struct CapabilityOfferRef<'a> {
    pub startup_parameters: &'a [FrontStartupParameter],
    pub shorthand: Option<(&'a str, &'a str)>,
    pub capability_id: &'a str,
    pub kind_id: &'a str,
    pub kind_contract_revision: &'a str,
    pub inputs: PortDescriptorRefs<'a>,
    pub outputs: PortDescriptorRefs<'a>,
    pub semantic_contract: &'a KindSemanticContract,
    pub execution_profile_id: &'a str,
    pub implementation_id: &'a str,
    pub artifact_id: &'a str,
    pub state_retention: Option<&'a StateRetentionSupport>,
    pub realization_properties: &'a [StructuredConfigurationValue],
    pub host_calls: HostCallRequirementRefs<'a>,
    pub resource_requirements: ResourceRequirementRefs<'a>,
    pub authority_requirements: AuthorityRequirementRefs<'a>,
    pub limits: &'a CapabilityLimits,
}
impl CapabilityOfferRef<'_> {
    /// The same semantic State retention gate for owned and borrowed offers.
    pub fn validate_state_retention(self) -> Result<(), StateRetentionSupportError> {
        if self.state_retention.is_some() && self.kind_id != crate::STATE_VALUE_KIND {
            return Err(StateRetentionSupportError::WrongSemanticKind);
        }
        Ok(())
    }
}
impl<'a> From<&'a CapabilityOffer> for CapabilityOfferRef<'a> {
    fn from(v: &'a CapabilityOffer) -> Self {
        Self {
            startup_parameters: &v.startup_parameters,
            shorthand: v.shorthand.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
            capability_id: v.capability_id.as_str(),
            kind_id: v.kind_id.as_str(),
            kind_contract_revision: v.kind_contract_revision.as_str(),
            inputs: PortDescriptorRefs::Owned(&v.inputs),
            outputs: PortDescriptorRefs::Owned(&v.outputs),
            semantic_contract: &v.semantic_contract,
            execution_profile_id: v.implementation.execution_profile_id.as_str(),
            implementation_id: v.implementation.implementation_id.as_str(),
            artifact_id: v.implementation.artifact_id.as_str(),
            state_retention: v.state_retention.as_ref(),
            realization_properties: &v.realization_properties,
            host_calls: HostCallRequirementRefs::Owned(&v.host_calls),
            resource_requirements: ResourceRequirementRefs::Owned(&v.resource_requirements),
            authority_requirements: AuthorityRequirementRefs::Owned(&v.authority_requirements),
            limits: &v.limits,
        }
    }
}
#[derive(Clone, Copy)]
pub enum CapabilityOfferRefs<'a> {
    Owned(&'a [CapabilityOffer]),
    Borrowed(&'a [CapabilityOfferRef<'a>]),
}
impl<'a> CapabilityOfferRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = CapabilityOfferRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub enum CapabilityIdRefs<'a> {
    Owned(&'a [CapabilityId]),
    Borrowed(&'a [&'a str]),
}
impl<'a> CapabilityIdRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a str> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => v[i].as_str(),
            Self::Borrowed(v) => v[i],
        })
    }
    pub fn contains(self, id: &str) -> bool {
        self.iter().any(|candidate| candidate == id)
    }
}
#[derive(Clone, Copy)]
pub enum ResourcePoolIdRefs<'a> {
    Owned(&'a [ResourcePoolId]),
    Borrowed(&'a [&'a str]),
}
impl<'a> ResourcePoolIdRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a str> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => v[i].as_str(),
            Self::Borrowed(v) => v[i],
        })
    }
    pub fn contains(self, id: &str) -> bool {
        self.iter().any(|candidate| candidate == id)
    }
}
#[derive(Clone, Copy)]
pub struct BaseProviderAdvertisementRef<'a> {
    pub base_id: &'a str,
    pub provider_instance_id: &'a str,
    pub provider_generation: u64,
    pub implementation_id: &'a str,
    pub mechanism_family: &'a str,
    pub enforcement_class: BaseEnforcementClass,
    pub lifecycle: BaseLifecycle,
    pub capability_ids: CapabilityIdRefs<'a>,
    pub resource_pool_ids: ResourcePoolIdRefs<'a>,
}
impl<'a> From<&'a BaseProviderAdvertisement> for BaseProviderAdvertisementRef<'a> {
    fn from(v: &'a BaseProviderAdvertisement) -> Self {
        Self {
            base_id: v.base_id.as_str(),
            provider_instance_id: v.provider_instance_id.as_str(),
            provider_generation: v.provider_generation,
            implementation_id: v.implementation_id.as_str(),
            mechanism_family: v.mechanism_family.as_str(),
            enforcement_class: v.enforcement_class,
            lifecycle: v.lifecycle,
            capability_ids: CapabilityIdRefs::Owned(&v.capability_ids),
            resource_pool_ids: ResourcePoolIdRefs::Owned(&v.resource_pool_ids),
        }
    }
}
#[derive(Clone, Copy)]
pub enum BaseProviderAdvertisementRefs<'a> {
    Owned(&'a [BaseProviderAdvertisement]),
    Borrowed(&'a [BaseProviderAdvertisementRef<'a>]),
}
impl<'a> BaseProviderAdvertisementRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = BaseProviderAdvertisementRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub struct HostAdvertisementRef<'a> {
    pub protocol_version: u16,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: OfferGeneration,
    pub profile: &'a str,
    pub bases: BaseProviderAdvertisementRefs<'a>,
    pub resources: ResourceOfferRefs<'a>,
    pub capabilities: CapabilityOfferRefs<'a>,
    pub planner_capabilities: &'a [PlannerCapabilityOffer],
}
impl<'a> From<&'a HostAdvertisement> for HostAdvertisementRef<'a> {
    fn from(v: &'a HostAdvertisement) -> Self {
        Self {
            protocol_version: v.protocol_version,
            host_id: v.host_id.as_str(),
            boot_id: v.boot_id.as_str(),
            offer_generation: v.offer_generation,
            profile: v.profile.as_str(),
            bases: BaseProviderAdvertisementRefs::Owned(&v.bases),
            resources: ResourceOfferRefs::Owned(&v.resources),
            capabilities: CapabilityOfferRefs::Owned(&v.capabilities),
            planner_capabilities: &v.planner_capabilities,
        }
    }
}
/// Complete borrowed semantic definition. This is a description, not a validated
/// Kind or permission to offer an implementation. `validate` shares the complete
/// ordinary Kind validator without materializing metadata; validation grants no
/// actual provider custody or permission to execute an operation.
#[derive(Clone, Copy)]
pub struct KindDefinitionRef<'a> {
    pub startup_parameters: &'a [FrontStartupParameter],
    pub shorthand: Option<(&'a str, &'a str)>,
    pub kind_id: &'a str,
    pub kind_contract_revision: &'a str,
    pub inputs: PortDescriptorRefs<'a>,
    pub outputs: PortDescriptorRefs<'a>,
    pub configuration: &'a [KindConfigurationField],
    pub semantic_laws: &'a [KindSemanticLaw],
    pub limits: &'a CapabilityLimits,
}
impl<'a> From<&'a Kind> for KindDefinitionRef<'a> {
    fn from(kind: &'a Kind) -> Self {
        Self {
            startup_parameters: &kind.startup_parameters,
            shorthand: kind
                .shorthand
                .as_ref()
                .map(|(a, b)| (a.as_str(), b.as_str())),
            kind_id: kind.kind_id.as_str(),
            kind_contract_revision: kind.kind_contract_revision.as_str(),
            inputs: PortDescriptorRefs::Owned(&kind.inputs),
            outputs: PortDescriptorRefs::Owned(&kind.outputs),
            configuration: &kind.configuration,
            semantic_laws: &kind.semantic_laws,
            limits: &kind.limits,
        }
    }
}
impl KindDefinitionRef<'_> {
    pub fn value_contracts(&self) -> &[crate::FrontValueContract] {
        self.semantic_laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::ValueContracts(contracts) => Some(contracts.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }
    /// Reconstruct every field without granting validation or native custody.
    pub fn materialize(self) -> Kind {
        Kind {
            startup_parameters: self.startup_parameters.to_vec(),
            shorthand: self.shorthand.map(|(a, b)| (a.into(), b.into())),
            kind_id: self.kind_id.into(),
            kind_contract_revision: self.kind_contract_revision.into(),
            inputs: self.inputs.iter().map(Into::into).collect(),
            outputs: self.outputs.iter().map(Into::into).collect(),
            configuration: self.configuration.to_vec(),
            semantic_laws: self.semantic_laws.to_vec(),
            limits: self.limits.clone(),
        }
    }
}

/// Complete descriptive authority grant; only actual Root possession can issue
/// a finite executable table. This view cannot authorize an effect.
#[derive(Clone, Copy)]
pub struct AuthorityGrantRef<'a> {
    pub grant_id: &'a str,
    pub contract_id: &'a str,
    pub host_call_contract_id: &'a str,
    pub subject_kind: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub capability_id: &'a str,
}
impl<'a> From<&'a AuthorityGrant> for AuthorityGrantRef<'a> {
    fn from(grant: &'a AuthorityGrant) -> Self {
        Self {
            grant_id: grant.grant_id.as_str(),
            contract_id: grant.contract_id.as_str(),
            host_call_contract_id: grant.host_call_contract_id.as_str(),
            subject_kind: grant.subject_kind.as_str(),
            host_id: grant.host_id.as_str(),
            boot_id: grant.boot_id.as_str(),
            capability_id: grant.capability_id.as_str(),
        }
    }
}
impl<'a> From<AuthorityGrantRef<'a>> for AuthorityBindingRef<'a> {
    fn from(grant: AuthorityGrantRef<'a>) -> Self {
        Self {
            grant_id: grant.grant_id,
            contract_id: grant.contract_id,
            host_call_contract_id: grant.host_call_contract_id,
            subject_kind: grant.subject_kind,
            host_id: grant.host_id,
            boot_id: grant.boot_id,
            capability_id: grant.capability_id,
        }
    }
}
#[derive(Clone, Copy)]
pub enum AuthorityGrantRefs<'a> {
    Owned(&'a [AuthorityGrant]),
    Borrowed(&'a [AuthorityGrantRef<'a>]),
}
impl<'a> AuthorityGrantRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = AuthorityGrantRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Full current descriptive entry. Validation remains the ordinary registry's
/// non-authorizing identity/count/projection predicates, shared with rich input.
#[derive(Clone, Copy)]
pub struct BaseProviderEntryRef<'a> {
    pub base_id: &'a str,
    pub provider_instance_id: &'a str,
    pub provider_generation: u64,
    pub implementation_id: &'a str,
    pub mechanism_family: &'a str,
    pub enforcement_class: BaseEnforcementClass,
    pub lifecycle: BaseLifecycle,
    pub capabilities: CapabilityOfferRefs<'a>,
    pub resources: ResourceOfferRefs<'a>,
}
impl<'a> From<&'a BaseProviderEntry> for BaseProviderEntryRef<'a> {
    fn from(entry: &'a BaseProviderEntry) -> Self {
        Self {
            base_id: entry.base_id.as_str(),
            provider_instance_id: entry.provider_instance_id.as_str(),
            provider_generation: entry.provider_generation,
            implementation_id: entry.implementation_id.as_str(),
            mechanism_family: entry.mechanism_family.as_str(),
            enforcement_class: entry.enforcement_class,
            lifecycle: entry.lifecycle,
            capabilities: CapabilityOfferRefs::Owned(&entry.capabilities),
            resources: ResourceOfferRefs::Owned(&entry.resources),
        }
    }
}
