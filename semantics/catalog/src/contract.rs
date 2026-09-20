//! Portable kind contracts and their finite configuration/terminal behavior.
use alloc::{string::String, vec::Vec};
use conduit_core::{CapabilityLimits, ConfigurationValue, Kind, KindId, PortDescriptor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalBehavior {
    EmitsOnce,
    EmitsOnceWhenScopeIsEligible,
    CompletesAfterConfiguredCount,
    CompletesAfterFixedCount { count: u64 },
    CompletesWhenInputsClose,
    MirrorsInputTerminal,
    RetainsLatestUntilReleased,
    EmitsCurrentAndCompletesWhenInputCloses,
    CoupledAtomicFanoutAndMirrorsInputTerminal,
    CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose,
    CurrentScalarSelectorCompletesWhenInputsClose,
    EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
    TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
    InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
    DelaysEachValueInOrderAndDrainsOnInputClosure,
    LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
    SimulatedCurrentObservationEmitsOnce,
    HostInputEndsOrFailsSource,
    HostObservationEndsOrFailsSource,
    EmitsInitialAndTogglesUntilInputCloses,
    EmitsOneField,
    EvolvesAfterTicksAndCompletesWhenTickCloses,
    PresentsEachFieldAndCompletesWhenInputCloses,
    CompletesAfterDockedRefusedOrDeadline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardConfigurationField {
    pub key: String,
    pub default_value: ConfigurationValue,
    pub rule: StandardConfigurationRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StandardConfigurationRule {
    Any,
    U64Range {
        minimum: u64,
        maximum: u64,
    },
    I64Range {
        minimum: i64,
        maximum: i64,
    },
    DurationMillis {
        minimum: u64,
        maximum: u64,
    },
    QuantityRange {
        minimum: i64,
        maximum: i64,
        canonical_unit: conduit_core::QuantityUnit,
    },
    TextBytes {
        maximum: u32,
    },
    TextOneOf {
        values: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardKindContract {
    pub kind_id: KindId,
    pub plain_name: String,
    pub summary: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<StandardConfigurationField>,
    pub limits: CapabilityLimits,
    pub terminal_behavior: TerminalBehavior,
    pub hosted_implementation_required: bool,
    pub browser_manifestation_honest: bool,
    pub pico_manifestation_honest: bool,
    pub example: String,
}

impl StandardKindContract {
    pub fn into_semantic_contract(self, revision: &str) -> Kind {
        Kind {
            startup_parameters: crate::startup_front(&self.configuration),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: revision.into(),
            inputs: self.inputs,
            outputs: self.outputs,
            limits: self.limits,
        }
    }
}
