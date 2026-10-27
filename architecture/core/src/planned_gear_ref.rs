//! Borrowed canonical records. These are descriptions, never verification or authority.
use crate::*;

#[derive(Clone, Copy)]
pub struct BaseProviderBindingRef<'a> {
    pub base_id: &'a str,
    pub provider_instance_id: &'a str,
    pub provider_generation: u64,
    pub implementation_id: &'a str,
    pub mechanism_family: &'a str,
    pub enforcement_class: BaseEnforcementClass,
}
impl<'a> From<&'a BaseProviderBinding> for BaseProviderBindingRef<'a> {
    fn from(v: &'a BaseProviderBinding) -> Self {
        Self {
            base_id: v.base_id.as_str(),
            provider_instance_id: v.provider_instance_id.as_str(),
            provider_generation: v.provider_generation,
            implementation_id: v.implementation_id.as_str(),
            mechanism_family: v.mechanism_family.as_str(),
            enforcement_class: v.enforcement_class,
        }
    }
}
#[derive(Clone, Copy)]
pub struct ResourceBindingRef<'a> {
    pub pool_id: &'a str,
    pub class_id: &'a str,
    pub units: u32,
    pub protected: Option<&'a ProtectedResourceBinding>,
    pub compute: Option<&'a ComputeReservation>,
    pub content: Option<&'a ResourceContentOffer>,
}
impl<'a> From<&'a ResourceBinding> for ResourceBindingRef<'a> {
    fn from(v: &'a ResourceBinding) -> Self {
        Self {
            pool_id: v.pool_id.as_str(),
            class_id: v.class_id.as_str(),
            units: v.units,
            protected: v.protected.as_ref(),
            compute: v.compute.as_ref(),
            content: v.content.as_ref(),
        }
    }
}
#[derive(Clone, Copy)]
pub struct AuthorityBindingRef<'a> {
    pub grant_id: &'a str,
    pub contract_id: &'a str,
    pub host_call_contract_id: &'a str,
    pub subject_kind: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub capability_id: &'a str,
}
impl<'a> From<&'a AuthorityBinding> for AuthorityBindingRef<'a> {
    fn from(v: &'a AuthorityBinding) -> Self {
        Self {
            grant_id: v.grant_id.as_str(),
            contract_id: v.contract_id.as_str(),
            host_call_contract_id: v.host_call_contract_id.as_str(),
            subject_kind: v.subject_kind.as_str(),
            host_id: v.host_id.as_str(),
            boot_id: v.boot_id.as_str(),
            capability_id: v.capability_id.as_str(),
        }
    }
}
/// Either existing owned records or independently assembled borrowed records.
#[derive(Clone, Copy)]
pub enum ResourceBindingRefs<'a> {
    Owned(&'a [ResourceBinding]),
    Borrowed(&'a [ResourceBindingRef<'a>]),
}
impl<'a> ResourceBindingRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = ResourceBindingRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Either existing owned records or independently assembled borrowed records.
#[derive(Clone, Copy)]
pub enum AuthorityBindingRefs<'a> {
    Owned(&'a [AuthorityBinding]),
    Borrowed(&'a [AuthorityBindingRef<'a>]),
}
impl<'a> AuthorityBindingRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = AuthorityBindingRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
/// Complete borrowed placement fingerprint input. Public construction confers
/// no validation, native possession, Plan admission or effect permission.
#[derive(Clone, Copy)]
pub struct PlannedGearRef<'a> {
    pub placement_id: &'a str,
    pub gear_id: &'a str,
    pub kind_id: &'a str,
    pub kind_contract_revision: &'a str,
    pub execution_profile_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub capability_id: &'a str,
    pub implementation_id: &'a str,
    pub artifact_id: &'a str,
    pub source_span: Option<SourceSpan>,
    pub offer_generation: OfferGeneration,
    pub configuration: &'a [ConfigurationEntry],
    pub base: Option<BaseProviderBindingRef<'a>>,
    pub realization_characteristics: &'a [RealizationCharacteristic],
    pub realization_properties: &'a [StructuredConfigurationValue],
    pub limits: &'a CapabilityLimits,
    pub inputs: &'a [PortDescriptor],
    pub outputs: &'a [PortDescriptor],
    pub semantic_contract: &'a KindSemanticContract,
    pub terminal_transductions: &'a [TerminalTransductionProfile],
    pub host_calls: &'a [HostCallRequirement],
    pub resources: ResourceBindingRefs<'a>,
    pub authority: AuthorityBindingRefs<'a>,
    pub pool_references: &'a [SharedPoolId],
}
impl<'a> From<&'a PlannedGear> for PlannedGearRef<'a> {
    fn from(v: &'a PlannedGear) -> Self {
        Self {
            placement_id: v.placement_id.as_str(),
            gear_id: v.gear_id.as_str(),
            kind_id: v.kind_id.as_str(),
            kind_contract_revision: v.kind_contract_revision.as_str(),
            execution_profile_id: v.execution_profile_id.as_str(),
            host_id: v.host_id.as_str(),
            boot_id: v.boot_id.as_str(),
            capability_id: v.capability_id.as_str(),
            implementation_id: v.implementation_id.as_str(),
            artifact_id: v.artifact_id.as_str(),
            source_span: v.source_span,
            offer_generation: v.offer_generation,
            base: v.base.as_ref().map(Into::into),
            resources: ResourceBindingRefs::Owned(&v.resources),
            authority: AuthorityBindingRefs::Owned(&v.authority),
            configuration: &v.configuration,
            realization_characteristics: &v.realization_characteristics,
            realization_properties: &v.realization_properties,
            limits: &v.limits,
            inputs: &v.inputs,
            outputs: &v.outputs,
            semantic_contract: &v.semantic_contract,
            terminal_transductions: &v.terminal_transductions,
            host_calls: &v.host_calls,
            pool_references: &v.pool_references,
        }
    }
}
