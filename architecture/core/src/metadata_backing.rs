//! Reachable owned capacities of the reviewed semantic Kind/Fore metadata.
//! Payload only: inline Self and allocator overhead are separate. Inspection
//! allocates nothing and confers no checking, reservation or effect authority.
use crate::*;
use alloc::{string::String, vec::Vec};
use core::mem::size_of;

#[derive(Default)]
pub(crate) struct Bytes(pub(crate) usize);
impl Bytes {
    pub(crate) fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    pub(crate) fn strings(&mut self, values: &[&String]) -> Option<()> {
        for value in values {
            self.add(value.capacity())?;
        }
        Some(())
    }
    pub(crate) fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }
    pub(crate) fn startup(&mut self, values: &Vec<FrontStartupParameter>) -> Option<()> {
        self.vector(values)?;
        for FrontStartupParameter {
            name,
            value_type,
            has_default: _,
        } in values
        {
            self.strings(&[name, &value_type.0])?;
        }
        Some(())
    }
    pub(crate) fn ports(&mut self, values: &Vec<PortDescriptor>) -> Option<()> {
        self.vector(values)?;
        for PortDescriptor {
            port_id,
            value_kind,
            direction: _,
            temporal: _,
            abnormal_kind,
        } in values
        {
            self.strings(&[&port_id.0, &value_kind.0])?;
            if let Some(kind) = abnormal_kind {
                self.add(kind.0.capacity())?;
            }
        }
        Some(())
    }
    pub(crate) fn shorthand(&mut self, value: &Option<(PortId, PortId)>) -> Option<()> {
        if let Some((input, output)) = value {
            self.strings(&[&input.0, &output.0])?;
        }
        Some(())
    }
    pub(crate) fn resources(&mut self, values: &Vec<ResourcePortContract>) -> Option<()> {
        self.vector(values)?;
        for ResourcePortContract {
            port_id,
            class_id,
            ownership: _,
            lifecycle: _,
            mobility: _,
        } in values
        {
            self.strings(&[&port_id.0, &class_id.0])?;
        }
        Some(())
    }
    pub(crate) fn values(&mut self, values: &Vec<FrontValueContract>) -> Option<()> {
        self.vector(values)?;
        for FrontValueContract { location, contract } in values {
            self.add(match location {
                FrontValueLocation::Startup(name) => name.capacity(),
                FrontValueLocation::Input(id)
                | FrontValueLocation::Output(id)
                | FrontValueLocation::InputAbnormal(id)
                | FrontValueLocation::OutputAbnormal(id) => id.0.capacity(),
            })?;
            self.add(contract.retained_backing_bytes()?)?;
        }
        Some(())
    }
    pub(crate) fn configuration_value(&mut self, value: &ConfigurationValue) -> Option<()> {
        match value {
            ConfigurationValue::Bool(_)
            | ConfigurationValue::U64(_)
            | ConfigurationValue::I64(_) => Some(()),
            ConfigurationValue::Text(text) => self.add(text.capacity()),
            ConfigurationValue::Structured(value) => self.add(value.owned_backing_bytes()?),
            // These checked wrappers retain private canonical/source capacities;
            // slices/serialized lengths cannot stand in for their ownership.
            ConfigurationValue::Quantity(_)
            | ConfigurationValue::Unit(_)
            | ConfigurationValue::TemperatureDifference(_) => None,
        }
    }
    pub(crate) fn configuration_rule(&mut self, rule: &KindConfigurationRule) -> Option<()> {
        match rule {
            KindConfigurationRule::Any
            | KindConfigurationRule::U64Range { .. }
            | KindConfigurationRule::I64Range { .. }
            | KindConfigurationRule::DurationMillis { .. }
            | KindConfigurationRule::TextBytes { .. }
            | KindConfigurationRule::DataLabel { .. }
            | KindConfigurationRule::Quantity
            | KindConfigurationRule::Unit
            | KindConfigurationRule::TemperatureDifference => Some(()),
            KindConfigurationRule::Structured { profile } => self.add(profile.0.capacity()),
            KindConfigurationRule::TextOneOf { values } => {
                self.vector(values)?;
                for text in values {
                    self.add(text.capacity())?;
                }
                Some(())
            }
            // Unit is Copy fixed-capacity state; this rule owns one Box<Unit>.
            KindConfigurationRule::QuantityRange { .. } => self.add(size_of::<Unit>()),
        }
    }
    pub(crate) fn laws(&mut self, laws: &Vec<KindSemanticLaw>) -> Option<()> {
        self.vector(laws)?;
        for law in laws {
            match law {
                KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                    property_profile,
                    configuration_key,
                })
                | KindSemanticLaw::RealizationRequirement {
                    property_profile,
                    configuration_key,
                } => self.strings(&[&property_profile.0, configuration_key])?,
                KindSemanticLaw::Terminal(value) => terminal(value),
                KindSemanticLaw::ExternalEffects(_)
                | KindSemanticLaw::TemporalState(_)
                | KindSemanticLaw::TimeDependence(_)
                | KindSemanticLaw::RandomDependence(_)
                | KindSemanticLaw::ResourceDependence(_)
                | KindSemanticLaw::Suspension(_)
                | KindSemanticLaw::Variability(_) => (),
                KindSemanticLaw::Replay(value) => match value {
                    ReplayBehavior::Exact | ReplayBehavior::Ineligible => (),
                    ReplayBehavior::Idempotent { operation_key_kind } => {
                        self.add(operation_key_kind.0.capacity())?
                    }
                    ReplayBehavior::Transactional {
                        transaction_contract,
                    } => self.add(transaction_contract.0.capacity())?,
                    ReplayBehavior::Compensatable {
                        compensation_contract,
                    } => self.add(compensation_contract.0.capacity())?,
                },
                KindSemanticLaw::ResourcePorts(ports) => self.resources(ports)?,
                KindSemanticLaw::ValueContracts(values) => self.values(values)?,
                // Flow/state/correlation and terminal-transduction descendants
                // are not required by the configured native profile; refuse.
                KindSemanticLaw::TerminalTransduction(_)
                | KindSemanticLaw::KeyedJoin(_)
                | KindSemanticLaw::BoundedCollect(_)
                | KindSemanticLaw::FlowEach(_)
                | KindSemanticLaw::FlowSelect(_)
                | KindSemanticLaw::FlowFold(_)
                | KindSemanticLaw::FlowScan(_)
                | KindSemanticLaw::InitializedState(_) => return None,
            }
        }
        Some(())
    }
}
// All current terminal variants are inline. Enumerate them so adding an owned
// terminal descendant cannot silently preserve a zero-byte accounting claim.
fn terminal(value: &KindTerminalBehavior) {
    use KindTerminalBehavior::*;
    match value {
        EmitsOnce
        | EmitsOnceWhenScopeIsEligible
        | CompletesAfterConfiguredCount
        | CompletesAfterFixedCount { count: _ }
        | CompletesWhenInputsClose
        | MirrorsInputTerminal
        | RetainsLatestUntilReleased
        | EmitsThroughSourceTerminalFlag
        | EmitsCurrentAndCompletesWhenInputCloses
        | CoupledAtomicFanoutAndMirrorsInputTerminal
        | FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner
        | CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose
        | CurrentScalarSelectorCompletesWhenInputsClose
        | EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible
        | TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses
        | InactivityStateCancelsDeadlineAndCompletesWhenInputCloses
        | DelaysEachValueInOrderAndDrainsOnInputClosure
        | LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses
        | SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses
        | TumblingProcessingTimeWindow { maximum_items: _ }
        | SimulatedCurrentObservationEmitsOnce
        | HostInputEndsOrFailsSource
        | HostObservationEndsOrFailsSource
        | EmitsInitialAndTogglesUntilInputCloses
        | EmitsOneField
        | EvolvesAfterTicksAndCompletesWhenTickCloses
        | PresentsEachFieldAndCompletesWhenInputCloses
        | CompletesAfterDockedRefusedOrDeadline => (),
    }
}

#[cfg(test)]
mod tests;
