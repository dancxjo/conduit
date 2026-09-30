use crate::{
    ArtifactId, AuthorityRequirement, CancellationTransduction, CapabilityId, CapabilityLimits,
    CapabilityOffer, CheckedFront, ExecutionProfileId, FrontStartupParameter, HostCallRequirement,
    ImplementationId, ImplementationOffer, KindConfigurationField, KindId, KindIdentity,
    KindSemanticLaw, PortDescriptor, PortId, ResourceRequirement,
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
    pub fn value_contracts(&self) -> &[crate::FrontValueContract] {
        self.semantic_laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::ValueContracts(contracts) => Some(contracts.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn resource_ports(&self) -> &[crate::ResourcePortContract] {
        self.semantic_laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::ResourcePorts(ports) => Some(ports.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn semantic_contract(&self) -> crate::KindSemanticContract {
        crate::KindSemanticContract {
            configuration: self.configuration.clone(),
            laws: self.semantic_laws.clone(),
        }
    }

    pub fn terminal_transductions(
        &self,
    ) -> impl Iterator<Item = &crate::TerminalTransductionProfile> {
        self.semantic_laws.iter().filter_map(|law| match law {
            KindSemanticLaw::TerminalTransduction(profile) => Some(profile),
            _ => None,
        })
    }

    pub fn keyed_join(&self) -> Option<&crate::KeyedJoinSemanticLaw> {
        self.semantic_laws.iter().find_map(|law| match law {
            KindSemanticLaw::KeyedJoin(contract) => Some(contract),
            _ => None,
        })
    }

    pub fn bounded_collect(&self) -> Option<&crate::BoundedCollectSemanticLaw> {
        self.semantic_laws.iter().find_map(|law| match law {
            KindSemanticLaw::BoundedCollect(contract) => Some(contract),
            _ => None,
        })
    }

    pub fn checked_front(&self) -> CheckedFront {
        CheckedFront::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
        .with_resource_ports(self.resource_ports().to_vec())
        .with_value_contracts(self.value_contracts().to_vec())
    }

    pub fn validate(&self) -> Result<(), KindValidationError> {
        if self.kind_id.as_str().is_empty() {
            return Err(KindValidationError::EmptyId);
        }
        if self.kind_contract_revision.as_str().is_empty() {
            return Err(KindValidationError::EmptyIdentity);
        }
        if self.inputs.iter().chain(&self.outputs).any(|port| {
            port.abnormal_kind
                .as_ref()
                .is_some_and(|kind| kind.as_str().is_empty())
        }) {
            return Err(KindValidationError::EmptyAbnormalTerminalKind);
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
            let semantic_kind = match (&field.rule, &field.default_value) {
                (crate::KindConfigurationRule::Any, _) => front.value_type.clone(),
                (
                    crate::KindConfigurationRule::QuantityRange { canonical_unit, .. },
                    crate::ConfigurationValue::Quantity(_),
                ) => crate::kind_id(canonical_unit.dimension().info_id()),
                (
                    crate::KindConfigurationRule::DurationMillis { .. },
                    crate::ConfigurationValue::U64(_),
                ) => crate::kind_id(crate::DURATION_INFO_ID),
                _ => field.default_value.semantic_kind(),
            };
            if front.value_type != semantic_kind {
                return Err(KindValidationError::ConfigurationFrontMismatch);
            }
        }
        let mut terminal_inputs = BTreeSet::new();
        let mut previous_terminal_input = None;
        let mut cancellation_behavior = None;
        let mut cancellation_request = None;
        let mut resource_ports = None;
        let mut value_contracts = None;
        let mut keyed_join = None;
        let mut bounded_collect = None;
        let mut flow_select = None;
        let mut flow_fold = None;
        let mut flow_each = None;
        for law in &self.semantic_laws {
            match law {
                KindSemanticLaw::TerminalTransduction(profile) => {
                    if !terminal_inputs.insert(profile.input_port_id.as_str()) {
                        return Err(KindValidationError::DuplicateTerminalTransduction);
                    }
                    if previous_terminal_input
                        .is_some_and(|previous: &str| previous >= profile.input_port_id.as_str())
                    {
                        return Err(KindValidationError::NonCanonicalTerminalTransductionOrder);
                    }
                    previous_terminal_input = Some(profile.input_port_id.as_str());
                    validate_terminal_transduction(self, profile)?;
                    if cancellation_behavior
                        .as_ref()
                        .is_some_and(|established| *established != &profile.cancellation)
                    {
                        return Err(KindValidationError::ConflictingCancellationTransduction);
                    }
                    cancellation_behavior = Some(&profile.cancellation);
                    if matches!(
                        profile.cancellation,
                        CancellationTransduction::Request { .. }
                    ) {
                        let request = (&profile.output_port_id, &profile.cancellation);
                        if cancellation_request
                            .as_ref()
                            .is_some_and(|established| *established != request)
                        {
                            return Err(KindValidationError::ConflictingCancellationTransduction);
                        }
                        cancellation_request = Some(request);
                    }
                }
                KindSemanticLaw::ResourcePorts(ports) => {
                    if resource_ports.replace(ports).is_some() {
                        return Err(KindValidationError::DuplicateResourcePorts);
                    }
                    validate_resource_ports(self, ports)?;
                }
                KindSemanticLaw::ValueContracts(contracts) => {
                    if value_contracts.replace(contracts).is_some() {
                        return Err(KindValidationError::DuplicateValueBounds);
                    }
                    validate_value_contracts(self, contracts)?;
                }
                KindSemanticLaw::KeyedJoin(contract) => {
                    if keyed_join.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateKeyedJoin);
                    }
                    if contract.maximum_pending_per_side == 0
                        || contract.key.validate_definition().is_err()
                        || contract.left_value.validate_definition().is_err()
                        || contract.right_value.validate_definition().is_err()
                    {
                        return Err(KindValidationError::InvalidKeyedJoin);
                    }
                }
                KindSemanticLaw::BoundedCollect(contract) => {
                    if bounded_collect.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateBoundedCollect);
                    }
                    validate_bounded_collect(self, contract)?;
                }
                KindSemanticLaw::FlowSelect(contract) => {
                    if flow_select.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateFlowSelect);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output))
                        if input.temporal == crate::PortTemporal::Flow { closes: true }
                            && output.temporal == crate::PortTemporal::Flow { closes: true }
                            && input.value_kind == output.value_kind
                            && input.value_kind == contract.predicate_input_kind
                            && contract.predicate_output_kind.as_str() == crate::BOOL_INFO_ID
                            && contract.maximum_active == 1
                            && contract.maximum_queued == 1
                            && contract.maximum_items > 0)
                    {
                        return Err(KindValidationError::InvalidFlowSelect);
                    }
                }
                KindSemanticLaw::FlowFold(contract) => {
                    if flow_fold.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateFlowFold);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output))
                        if input.temporal == crate::PortTemporal::Flow { closes: true }
                            && output.temporal == crate::PortTemporal::Value
                            && input.value_kind == contract.item.value_kind
                            && output.value_kind == contract.accumulator.value_kind
                            && contract.item.validate_definition().is_ok()
                            && contract.accumulator.validate_definition().is_ok()
                            && contract.accumulator.validate(&contract.initial_accumulator).is_ok()
                            && contract.combine_accumulator_port_id.as_str() == "accumulator"
                            && contract.combine_item_port_id.as_str() == "item"
                            && contract.combine_output_port_id.as_str() == "combined"
                            && contract.maximum_active == 1
                            && contract.maximum_queued == 1
                            && contract.maximum_items > 0)
                    {
                        return Err(KindValidationError::InvalidFlowFold);
                    }
                }
                KindSemanticLaw::FlowEach(contract) => {
                    if flow_each.replace(contract).is_some() || contract.maximum_items == 0 {
                        return Err(KindValidationError::InvalidFlowEach);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output)) if input.temporal == crate::PortTemporal::Flow { closes: true } && output.temporal == crate::PortTemporal::Flow { closes: true })
                    {
                        return Err(KindValidationError::InvalidFlowEach);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn validate_bounded_collect(
    kind: &Kind,
    contract: &crate::BoundedCollectSemanticLaw,
) -> Result<(), KindValidationError> {
    let input = kind
        .inputs
        .iter()
        .find(|port| port.port_id == contract.input_port_id)
        .ok_or(KindValidationError::InvalidBoundedCollect)?;
    let output = kind
        .outputs
        .iter()
        .find(|port| port.port_id == contract.output_port_id)
        .ok_or(KindValidationError::InvalidBoundedCollect)?;
    if contract.maximum_items == 0
        || contract.element.validate_definition().is_err()
        || contract.collection.validate_definition().is_err()
        || contract.overflow_disposition.validate_definition().is_err()
        || input.direction != crate::PortDirection::Input
        || input.temporal != (crate::PortTemporal::Flow { closes: true })
        || input.value_kind != contract.element.value_kind
        || input.abnormal_kind.is_some()
        || output.direction != crate::PortDirection::Output
        || output.temporal != crate::PortTemporal::Value
        || output.value_kind != contract.collection.value_kind
        || output.abnormal_kind.as_ref() != Some(&contract.overflow_disposition.value_kind)
    {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    let encoder = crate::PreparedLeafSequenceEncoder::new(
        contract.element.value_kind.clone(),
        contract.element.maximum_bytes,
        contract.maximum_items,
    )
    .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    let value_type = encoder
        .value_type()
        .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    let profile = value_type
        .profile()
        .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    if profile.value_kind() != &contract.collection.value_kind
        || encoder.maximum_bytes() != contract.collection.maximum_bytes
    {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    let required = [
        (
            crate::FrontValueLocation::Input(contract.input_port_id.clone()),
            &contract.element,
        ),
        (
            crate::FrontValueLocation::Output(contract.output_port_id.clone()),
            &contract.collection,
        ),
        (
            crate::FrontValueLocation::OutputAbnormal(contract.output_port_id.clone()),
            &contract.overflow_disposition,
        ),
    ];
    if required.iter().any(|(location, expected)| {
        !kind
            .value_contracts()
            .iter()
            .any(|actual| actual.location == *location && actual.contract == **expected)
    }) {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    Ok(())
}

fn validate_value_contracts(
    kind: &Kind,
    contracts: &[crate::FrontValueContract],
) -> Result<(), KindValidationError> {
    let mut locations = BTreeSet::new();
    for value_contract in contracts {
        if !locations.insert(value_contract.location.clone()) {
            return Err(KindValidationError::InvalidValueBound);
        }
        let expected_kind = match &value_contract.location {
            crate::FrontValueLocation::Startup(name) => kind
                .startup_parameters
                .iter()
                .find(|parameter| &parameter.name == name)
                .map(|parameter| &parameter.value_type),
            crate::FrontValueLocation::Input(port) => kind
                .inputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .map(|candidate| &candidate.value_kind),
            crate::FrontValueLocation::Output(port) => kind
                .outputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .map(|candidate| &candidate.value_kind),
            crate::FrontValueLocation::InputAbnormal(port) => kind
                .inputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .and_then(|candidate| candidate.abnormal_kind.as_ref()),
            crate::FrontValueLocation::OutputAbnormal(port) => kind
                .outputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .and_then(|candidate| candidate.abnormal_kind.as_ref()),
        };
        let Some(expected_kind) = expected_kind else {
            return Err(KindValidationError::UnknownValueBoundLocation);
        };
        if expected_kind != &value_contract.contract.value_kind {
            return Err(KindValidationError::InvalidValueBound);
        }
    }
    Ok(())
}

fn validate_resource_ports(
    kind: &Kind,
    contracts: &[crate::ResourcePortContract],
) -> Result<(), KindValidationError> {
    let mut ids = BTreeSet::new();
    for contract in contracts {
        if contract.class_id.as_str().is_empty() {
            return Err(KindValidationError::EmptyResourcePortClass);
        }
        if !ids.insert(contract.port_id.as_str()) {
            return Err(KindValidationError::DuplicateResourcePort);
        }
        if !kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .any(|port| port.port_id == contract.port_id)
        {
            return Err(KindValidationError::UnknownResourcePort);
        }
    }
    Ok(())
}

fn validate_terminal_transduction(
    kind: &Kind,
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    validate_terminal_transduction_ports(&kind.inputs, &kind.outputs, profile)
}

pub(crate) fn validate_terminal_transduction_ports(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    use crate::{AbnormalTerminalTransduction, CancellationTransduction, NormalCloseTransduction};
    let input = inputs
        .iter()
        .find(|port| port.port_id == profile.input_port_id)
        .ok_or(KindValidationError::UnknownTerminalTransductionInput)?;
    let output = outputs
        .iter()
        .find(|port| port.port_id == profile.output_port_id)
        .ok_or(KindValidationError::UnknownTerminalTransductionOutput)?;
    if !matches!(&profile.normal_close, NormalCloseTransduction::NotAccepted)
        && (!matches!(input.temporal, crate::PortTemporal::Flow { closes: true })
            || !matches!(output.temporal, crate::PortTemporal::Flow { closes: true }))
    {
        return Err(KindValidationError::TerminalCloseModalityMismatch);
    }
    if !matches!(&profile.abnormal, AbnormalTerminalTransduction::NotAccepted)
        && input.abnormal_kind.is_none()
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    if matches!(
        &profile.abnormal,
        AbnormalTerminalTransduction::PropagateAfterDrain
            | AbnormalTerminalTransduction::FinalizeThenPropagate(_)
    ) && input.abnormal_kind != output.abnormal_kind
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    let close_bound = match &profile.normal_close {
        NormalCloseTransduction::FlushThenPropagate(bound)
        | NormalCloseTransduction::FlushThenPropagateWhenAllClose(bound) => Some(bound),
        _ => None,
    };
    let abnormal_bound = match &profile.abnormal {
        AbnormalTerminalTransduction::FinalizeThenPropagate(bound) => Some(bound),
        _ => None,
    };
    if close_bound
        .into_iter()
        .chain(abnormal_bound)
        .any(|bound| bound.maximum_items == 0 || bound.maximum_bytes == 0)
    {
        return Err(KindValidationError::EmptyTerminalEmissionBound);
    }
    let law_is_empty = match &profile.normal_close {
        NormalCloseTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.abnormal {
        AbnormalTerminalTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.cancellation {
        CancellationTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    };
    if law_is_empty {
        return Err(KindValidationError::EmptyTerminalLawIdentity);
    }
    if let CancellationTransduction::Request { disposition_kind } = &profile.cancellation {
        let cancellation_inputs = inputs
            .iter()
            .filter(|port| port.value_kind.as_str() == crate::CANCELLATION_REQUEST_INFO_ID)
            .count();
        if cancellation_inputs != 1 {
            return Err(KindValidationError::CancellationControlMismatch);
        }
        if output.abnormal_kind.as_ref() != Some(disposition_kind) {
            return Err(KindValidationError::CancellationDispositionMismatch);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindValidationError {
    EmptyId,
    EmptyIdentity,
    DuplicateConfigurationKey,
    ConfigurationMissingFromFront,
    ConfigurationFrontMismatch,
    EmptyAbnormalTerminalKind,
    DuplicateTerminalTransduction,
    NonCanonicalTerminalTransductionOrder,
    EmptyTerminalEmissionBound,
    EmptyTerminalLawIdentity,
    UnknownTerminalTransductionInput,
    UnknownTerminalTransductionOutput,
    TerminalCloseModalityMismatch,
    TerminalAbnormalKindMismatch,
    CancellationControlMismatch,
    CancellationDispositionMismatch,
    ConflictingCancellationTransduction,
    DuplicateResourcePorts,
    DuplicateResourcePort,
    EmptyResourcePortClass,
    UnknownResourcePort,
    DuplicateValueBounds,
    DuplicateKeyedJoin,
    InvalidKeyedJoin,
    DuplicateBoundedCollect,
    InvalidBoundedCollect,
    DuplicateFlowSelect,
    InvalidFlowSelect,
    DuplicateFlowFold,
    InvalidFlowFold,
    InvalidFlowEach,
    InvalidValueBound,
    UnknownValueBoundLocation,
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
    state_retention: Option<crate::StateRetentionSupport>,
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
            state_retention: None,
        }
    }

    pub fn try_new(contract: Kind, realization: Back) -> Result<Self, KindValidationError> {
        contract.validate()?;
        let realization_limits = contract.limits.clone();
        Ok(Self {
            contract,
            realization,
            realization_limits,
            state_retention: None,
        })
    }

    /// Advertise the longest keep duration this exact Back can truthfully
    /// satisfy. Only the canonical retained-State Kind may carry this fact.
    pub fn with_state_retention(
        mut self,
        support: crate::StateRetentionSupport,
    ) -> Result<Self, StateRetentionSupportError> {
        if self.contract.kind_id.as_str() != crate::STATE_VALUE_KIND {
            return Err(StateRetentionSupportError::WrongSemanticKind);
        }
        self.state_retention = Some(support);
        Ok(self)
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
        let semantic_contract = self.contract.semantic_contract();
        CapabilityOffer {
            startup_parameters: self.contract.startup_parameters,
            shorthand: self.contract.shorthand,
            capability_id: self.realization.capability_id,
            kind_id: self.contract.kind_id,
            kind_contract_revision: self.contract.kind_contract_revision,
            inputs: self.contract.inputs,
            outputs: self.contract.outputs,
            semantic_contract,
            implementation: ImplementationOffer {
                execution_profile_id: self.realization.execution_profile_id,
                implementation_id: self.realization.implementation_id,
                artifact_id: self.realization.artifact_id,
            },
            state_retention: self.state_retention,
            host_calls: self.realization.host_calls,
            resource_requirements: self.realization.resource_requirements,
            authority_requirements: self.realization.authority_requirements,
            limits: self.realization_limits,
        }
    }
}

impl CapabilityOffer {
    pub fn validate_state_retention(&self) -> Result<(), StateRetentionSupportError> {
        if self.state_retention.is_some() && self.kind_id.as_str() != crate::STATE_VALUE_KIND {
            return Err(StateRetentionSupportError::WrongSemanticKind);
        }
        Ok(())
    }

    pub fn with_state_retention(
        mut self,
        support: crate::StateRetentionSupport,
    ) -> Result<Self, StateRetentionSupportError> {
        self.state_retention = Some(support);
        self.validate_state_retention()?;
        Ok(self)
    }

    /// Revalidates the semantic half of an already assembled offer.
    ///
    /// Canonical production offers should use [`BackOfferBuilder`]. This is
    /// the checked boundary used by finite fixtures and composition code that
    /// must spell all wire fields explicitly: it prevents those callers from
    /// bypassing the same Kind validation merely because they do not own a
    /// reusable catalog value.
    #[doc(hidden)]
    pub fn validate_constructed_semantic_contract(&self) -> Result<(), KindValidationError> {
        Kind {
            startup_parameters: self.startup_parameters.clone(),
            shorthand: self.shorthand.clone(),
            kind_id: self.kind_id.clone(),
            kind_contract_revision: self.kind_contract_revision.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            configuration: self.semantic_contract.configuration.clone(),
            semantic_laws: self.semantic_contract.laws.clone(),
            limits: self.limits.clone(),
        }
        .validate()
    }
}

/// Checked named-field construction for finite fixtures and composition code.
///
/// Production Host catalogs should prefer [`BackOfferBuilder`], which derives
/// every semantic field directly from one canonical [`Kind`]. This macro is a
/// migration-safe boundary for callers that must spell the complete portable
/// record: unlike a raw literal, it always applies Kind validation.
#[macro_export]
macro_rules! capability_offer_from_parts {
    ($($fields:tt)*) => {{
        let offer = $crate::CapabilityOffer { state_retention: None, $($fields)* };
        if let Err(error) = offer.validate_constructed_semantic_contract() {
            panic!("CapabilityOffer requires a valid semantic contract: {:?}", error);
        }
        offer
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateRetentionSupportError {
    WrongSemanticKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PortDirection, PortTemporal, kind_id, port_id};
    use alloc::{format, string::ToString, vec};

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
                abnormal_kind: None,
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
    fn resource_ports_are_checked_fore_identity_without_bearer_material() {
        let mut resource = contract();
        resource
            .semantic_laws
            .push(KindSemanticLaw::ResourcePorts(vec![
                crate::ResourcePortContract {
                    port_id: port_id("in"),
                    class_id: crate::ResourceClassId::from("device/mmio-region"),
                    ownership: crate::ResourcePortOwnership::Move,
                    lifecycle: crate::ResourcePortLifecycle::Play,
                    mobility: Default::default(),
                },
            ]));
        resource.validate().unwrap();

        assert_eq!(resource.checked_front().resource_ports().len(), 1);
        assert_ne!(
            crate::compute_checked_front_fingerprint(&contract().checked_front()),
            crate::compute_checked_front_fingerprint(&resource.checked_front())
        );
    }

    #[test]
    fn resource_port_must_name_one_real_fore_port() {
        let mut resource = contract();
        resource
            .semantic_laws
            .push(KindSemanticLaw::ResourcePorts(vec![
                crate::ResourcePortContract {
                    port_id: port_id("missing"),
                    class_id: crate::ResourceClassId::from("device/mmio-region"),
                    ownership: crate::ResourcePortOwnership::Move,
                    lifecycle: crate::ResourcePortLifecycle::Play,
                    mobility: Default::default(),
                },
            ]));
        assert_eq!(
            resource.validate(),
            Err(KindValidationError::UnknownResourcePort)
        );
    }

    #[test]
    fn value_contracts_are_exact_checked_fore_identity_and_validate_fail_closed() {
        let mut bounded = contract();
        bounded
            .semantic_laws
            .push(KindSemanticLaw::ValueContracts(vec![
                crate::FrontValueContract {
                    location: crate::FrontValueLocation::Input(port_id("in")),
                    contract: crate::CheckedValueContract::new(kind_id("value/count"), 128, vec![])
                        .unwrap(),
                },
            ]));
        bounded.validate().unwrap();
        assert_eq!(
            bounded.checked_front().value_contracts()[0]
                .contract
                .maximum_bytes,
            128
        );
        assert_ne!(
            crate::compute_checked_front_fingerprint(&contract().checked_front()),
            crate::compute_checked_front_fingerprint(&bounded.checked_front())
        );
        let encoded = serde_json::to_vec(&bounded.checked_front()).unwrap();
        let decoded: crate::CheckedFront = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, bounded.checked_front());

        let mut wrong_kind = bounded.clone();
        let KindSemanticLaw::ValueContracts(contracts) = &mut wrong_kind.semantic_laws[0] else {
            unreachable!()
        };
        contracts[0].contract.value_kind = kind_id("value/text");
        assert_eq!(
            wrong_kind.validate(),
            Err(KindValidationError::InvalidValueBound)
        );

        let mut unknown = bounded;
        let KindSemanticLaw::ValueContracts(contracts) = &mut unknown.semantic_laws[0] else {
            unreachable!()
        };
        contracts[0].location = crate::FrontValueLocation::Input(port_id("missing"));
        assert_eq!(
            unknown.validate(),
            Err(KindValidationError::UnknownValueBoundLocation)
        );
    }

    #[test]
    fn abnormal_value_contract_must_name_the_exact_declared_terminal_kind() {
        let mut bounded = contract();
        bounded.inputs[0].abnormal_kind = Some(kind_id("value/text"));
        bounded
            .semantic_laws
            .push(KindSemanticLaw::ValueContracts(vec![
                crate::FrontValueContract {
                    location: crate::FrontValueLocation::InputAbnormal(port_id("in")),
                    contract: crate::CheckedValueContract::new(kind_id("value/text"), 24, vec![])
                        .unwrap(),
                },
            ]));
        assert_eq!(bounded.validate(), Ok(()));
        assert_eq!(
            bounded
                .checked_front()
                .value_contract(&crate::FrontValueLocation::InputAbnormal(port_id("in")))
                .unwrap()
                .maximum_bytes,
            24
        );

        let mut payload_kind = bounded.clone();
        let KindSemanticLaw::ValueContracts(contracts) =
            payload_kind.semantic_laws.last_mut().unwrap()
        else {
            unreachable!()
        };
        contracts[0].contract.value_kind = kind_id("value/count");
        assert_eq!(
            payload_kind.validate(),
            Err(KindValidationError::InvalidValueBound)
        );

        let mut undeclared = bounded;
        undeclared.inputs[0].abnormal_kind = None;
        assert_eq!(
            undeclared.validate(),
            Err(KindValidationError::UnknownValueBoundLocation)
        );
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
    fn terminal_transduction_keeps_types_and_three_behaviors_independent() {
        let mut contract = contract();
        contract.inputs[0].temporal = PortTemporal::Flow { closes: true };
        contract.inputs[0].abnormal_kind = Some(kind_id("test/work-terminal"));
        contract.inputs.push(PortDescriptor {
            port_id: port_id("halt"),
            direction: PortDirection::Input,
            value_kind: kind_id(crate::CANCELLATION_REQUEST_INFO_ID),
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        });
        contract.outputs.push(PortDescriptor {
            port_id: port_id("result"),
            direction: PortDirection::Output,
            value_kind: kind_id("value/count"),
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id("test/work-terminal")),
        });
        contract
            .semantic_laws
            .push(KindSemanticLaw::TerminalTransduction(
                crate::TerminalTransductionProfile {
                    input_port_id: port_id("in"),
                    output_port_id: port_id("result"),
                    normal_close: crate::NormalCloseTransduction::FlushThenPropagate(
                        crate::FiniteTerminalEmission {
                            maximum_items: 1,
                            maximum_bytes: 8,
                        },
                    ),
                    abnormal: crate::AbnormalTerminalTransduction::PropagateAfterDrain,
                    cancellation: crate::CancellationTransduction::Request {
                        disposition_kind: kind_id("test/work-terminal"),
                    },
                },
            ));
        assert_eq!(contract.validate(), Ok(()));

        let mut wrong_input = contract.clone();
        let KindSemanticLaw::TerminalTransduction(profile) = &mut wrong_input.semantic_laws[0]
        else {
            unreachable!()
        };
        profile.input_port_id = port_id("invented");
        assert_eq!(
            wrong_input.validate(),
            Err(KindValidationError::UnknownTerminalTransductionInput)
        );

        let mut wrong_terminal_kind = contract.clone();
        wrong_terminal_kind.inputs[0].abnormal_kind = Some(kind_id("test/other-terminal"));
        assert_eq!(
            wrong_terminal_kind.validate(),
            Err(KindValidationError::TerminalAbnormalKindMismatch)
        );

        for abnormal in [
            crate::AbnormalTerminalTransduction::Recover,
            crate::AbnormalTerminalTransduction::DomainSpecific {
                law: kind_id("test/recover-law"),
            },
        ] {
            let mut missing_abnormal_input = contract.clone();
            missing_abnormal_input.inputs[0].abnormal_kind = None;
            let KindSemanticLaw::TerminalTransduction(profile) =
                &mut missing_abnormal_input.semantic_laws[0]
            else {
                unreachable!()
            };
            profile.abnormal = abnormal;
            assert_eq!(
                missing_abnormal_input.validate(),
                Err(KindValidationError::TerminalAbnormalKindMismatch)
            );
        }

        let mut missing_control = contract.clone();
        missing_control
            .inputs
            .retain(|port| port.port_id.as_str() != "halt");
        assert_eq!(
            missing_control.validate(),
            Err(KindValidationError::CancellationControlMismatch)
        );

        let mut multiple = contract.clone();
        multiple.inputs.push(PortDescriptor {
            port_id: port_id("later"),
            direction: PortDirection::Input,
            value_kind: kind_id("value/count"),
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id("test/work-terminal")),
        });
        let KindSemanticLaw::TerminalTransduction(first) = multiple.semantic_laws.last().unwrap()
        else {
            unreachable!()
        };
        let mut second = first.clone();
        second.input_port_id = port_id("later");
        multiple
            .semantic_laws
            .push(KindSemanticLaw::TerminalTransduction(second));
        assert_eq!(multiple.validate(), Ok(()));

        let mut conflicting_cancellation = multiple.clone();
        let KindSemanticLaw::TerminalTransduction(second) =
            conflicting_cancellation.semantic_laws.last_mut().unwrap()
        else {
            unreachable!()
        };
        second.cancellation = crate::CancellationTransduction::NotCancellable;
        assert_eq!(
            conflicting_cancellation.validate(),
            Err(KindValidationError::ConflictingCancellationTransduction)
        );

        let mut noncanonical = multiple;
        let second = noncanonical.semantic_laws.pop().unwrap();
        noncanonical.semantic_laws.insert(0, second);
        assert_eq!(
            noncanonical.validate(),
            Err(KindValidationError::NonCanonicalTerminalTransductionOrder)
        );

        let mut unbounded_flush = contract;
        let KindSemanticLaw::TerminalTransduction(profile) =
            unbounded_flush.semantic_laws.last_mut().unwrap()
        else {
            unreachable!()
        };
        profile.normal_close =
            crate::NormalCloseTransduction::FlushThenPropagate(crate::FiniteTerminalEmission {
                maximum_items: 0,
                maximum_bytes: 8,
            });
        assert_eq!(
            unbounded_flush.validate(),
            Err(KindValidationError::EmptyTerminalEmissionBound)
        );
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

        kind.startup_parameters[0].value_type = kind_id(crate::DURATION_INFO_ID);
        kind.configuration[0].rule = crate::KindConfigurationRule::DurationMillis {
            minimum: 1,
            maximum: 8,
        };
        assert_eq!(kind.validate(), Ok(()));
        kind.startup_parameters[0].value_type = kind_id(crate::COUNT_INFO_ID);
        kind.configuration[0].rule = crate::KindConfigurationRule::U64Range {
            minimum: 1,
            maximum: 8,
        };

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

    #[test]
    fn semantic_contract_is_required_in_json_and_breaks_legacy_positional_frames_closed() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct LegacyCapabilityOffer {
            startup_parameters: Vec<FrontStartupParameter>,
            shorthand: Option<(PortId, PortId)>,
            capability_id: CapabilityId,
            kind_id: KindId,
            kind_contract_revision: KindIdentity,
            inputs: Vec<PortDescriptor>,
            outputs: Vec<PortDescriptor>,
            implementation: ImplementationOffer,
            host_calls: Vec<HostCallRequirement>,
            resource_requirements: Vec<ResourceRequirement>,
            authority_requirements: Vec<AuthorityRequirement>,
            limits: CapabilityLimits,
        }

        let offer = BackOfferBuilder::new(contract(), realization("wire-proof")).build();
        let mut json = serde_json::to_value(&offer).unwrap();
        json.as_object_mut().unwrap().remove("semantic_contract");
        assert!(serde_json::from_value::<CapabilityOffer>(json).is_err());

        let legacy = LegacyCapabilityOffer {
            startup_parameters: offer.startup_parameters.clone(),
            shorthand: offer.shorthand.clone(),
            capability_id: offer.capability_id.clone(),
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            implementation: offer.implementation.clone(),
            host_calls: offer.host_calls.clone(),
            resource_requirements: offer.resource_requirements.clone(),
            authority_requirements: offer.authority_requirements.clone(),
            limits: offer.limits.clone(),
        };
        let legacy_bytes = postcard::to_allocvec(&legacy).unwrap();
        assert!(postcard::from_bytes::<CapabilityOffer>(&legacy_bytes).is_err());

        // CapabilityOffer is a named-field JSON contract. Its flattened
        // implementation fields deliberately make positional serializers
        // refuse rather than silently assign a newly inserted field to an
        // older position. A future positional carrier needs its own versioned
        // envelope and cannot inherit this record layout.
        assert!(postcard::to_allocvec(&offer).is_err());
    }

    #[test]
    fn state_retention_is_offer_owned_validated_and_serialized() {
        let support = crate::StateRetentionSupport {
            maximum_lifetime: crate::StateLifetime::Body,
        };
        let ordinary = BackOfferBuilder::new(contract(), realization("ordinary")).build();
        assert_eq!(
            ordinary.clone().with_state_retention(support),
            Err(StateRetentionSupportError::WrongSemanticKind)
        );
        assert!(
            serde_json::to_value(&ordinary)
                .unwrap()
                .get("state_retention")
                .is_none()
        );

        let mut state_contract = contract();
        state_contract.kind_id = crate::kind_id(crate::STATE_VALUE_KIND);
        let offered = BackOfferBuilder::new(state_contract, realization("state"))
            .with_state_retention(support)
            .unwrap()
            .build();
        let json = serde_json::to_value(&offered).unwrap();
        assert_eq!(
            json.get("state_retention")
                .and_then(|value| value.get("maximum_lifetime"))
                .and_then(serde_json::Value::as_str),
            Some("Body")
        );
        let decoded: CapabilityOffer = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.state_retention, Some(support));
    }

    #[test]
    fn semantic_contract_integers_remain_exact_across_json_and_positional_carriers() {
        let contract = crate::KindSemanticContract {
            configuration: vec![KindConfigurationField {
                key: "range".into(),
                default_value: crate::ConfigurationValue::U64(u64::MAX),
                rule: crate::KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::MAX,
                },
            }],
            laws: vec![],
        };
        let json = serde_json::to_string(&contract).unwrap();
        assert!(json.contains(&format!("\"{}\"", u64::MAX)));
        assert_eq!(
            serde_json::from_str::<crate::KindSemanticContract>(&json).unwrap(),
            contract
        );

        let unsafe_number = json.replace(&format!("\"{}\"", u64::MAX), &u64::MAX.to_string());
        assert!(serde_json::from_str::<crate::KindSemanticContract>(&unsafe_number).is_err());

        let positional = postcard::to_allocvec(&contract).unwrap();
        assert_eq!(
            postcard::from_bytes::<crate::KindSemanticContract>(&positional).unwrap(),
            contract
        );
    }
}
