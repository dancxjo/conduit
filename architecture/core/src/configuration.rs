use alloc::{string::String, vec::Vec};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::{KindId, PortId, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructuredConfigurationValue {
    profile: KindId,
    canonical_value: Vec<u8>,
}

impl StructuredConfigurationValue {
    pub fn new(profile: KindId, canonical_value: Vec<u8>) -> Option<Self> {
        if profile.as_str().is_empty()
            || canonical_value.is_empty()
            || canonical_value.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return None;
        }
        let value = StructuredInfoValue::from_canonical_bytes(&canonical_value).ok()?;
        let actual_profile = value.value_type().profile().ok()?;
        (actual_profile.value_kind() == &profile).then_some(Self {
            profile,
            canonical_value,
        })
    }

    pub fn profile(&self) -> &KindId {
        &self.profile
    }

    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for StructuredConfigurationValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Encoded {
            profile: KindId,
            canonical_value: Vec<u8>,
        }

        let encoded = Encoded::deserialize(deserializer)?;
        Self::new(encoded.profile, encoded.canonical_value)
            .ok_or_else(|| D::Error::custom("invalid structured configuration value"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationValue {
    Bool(bool),
    U64(#[serde(with = "human_u64")] u64),
    /// Signed fixed-point scalar microunits, matching `value/scalar`.
    I64(#[serde(with = "human_i64")] i64),
    Text(String),
    /// Exact dimensional startup value; the unit remains part of configuration truth.
    Quantity(crate::Quantity),
    /// Exact finite structured semantic value used by an immutable Gear configuration.
    Structured(StructuredConfigurationValue),
}

impl ConfigurationValue {
    pub fn semantic_kind(&self) -> KindId {
        crate::kind_id(match self {
            Self::Bool(_) => crate::BOOL_INFO_ID,
            Self::U64(_) => crate::COUNT_INFO_ID,
            Self::I64(_) => crate::SCALAR_INFO_ID,
            Self::Text(_) => crate::TEXT_INFO_ID,
            Self::Quantity(_) => crate::QUANTITY_INFO_ID,
            Self::Structured(value) => value.profile().as_str(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationEntry {
    pub key: String,
    pub value: ConfigurationValue,
}

/// One immutable startup field owned by a semantic [`crate::Kind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindConfigurationField {
    pub key: String,
    pub default_value: ConfigurationValue,
    pub rule: KindConfigurationRule,
}

/// Exact portable semantic contract shared by every Back for one Kind.
///
/// The callable Fore is intentionally separate: equal port shape does not
/// permit a realization to change configuration or behavioral law.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindSemanticContract {
    pub configuration: Vec<KindConfigurationField>,
    pub laws: Vec<KindSemanticLaw>,
}

impl KindSemanticContract {
    pub fn is_empty(&self) -> bool {
        self.configuration.is_empty() && self.laws.is_empty()
    }

    pub fn value_contracts(&self) -> &[crate::FrontValueContract] {
        self.laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::ValueContracts(contracts) => Some(contracts.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }

    pub fn keyed_join(&self) -> Option<&KeyedJoinSemanticLaw> {
        self.laws.iter().find_map(|law| match law {
            KindSemanticLaw::KeyedJoin(contract) => Some(contract),
            _ => None,
        })
    }

    pub fn bounded_collect(&self) -> Option<&BoundedCollectSemanticLaw> {
        self.laws.iter().find_map(|law| match law {
            KindSemanticLaw::BoundedCollect(contract) => Some(contract),
            _ => None,
        })
    }
}

/// Finite validation law for one Kind configuration field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KindConfigurationRule {
    Any,
    U64Range {
        #[serde(with = "human_u64")]
        minimum: u64,
        #[serde(with = "human_u64")]
        maximum: u64,
    },
    I64Range {
        #[serde(with = "human_i64")]
        minimum: i64,
        #[serde(with = "human_i64")]
        maximum: i64,
    },
    DurationMillis {
        #[serde(with = "human_u64")]
        minimum: u64,
        #[serde(with = "human_u64")]
        maximum: u64,
    },
    QuantityRange {
        #[serde(with = "human_i64")]
        minimum: i64,
        #[serde(with = "human_i64")]
        maximum: i64,
        canonical_unit: crate::QuantityUnit,
    },
    TextBytes {
        maximum: u32,
    },
    TextOneOf {
        values: Vec<String>,
    },
    Structured {
        profile: KindId,
    },
}

const MAXIMUM_EXACT_JAVASCRIPT_INTEGER: u64 = 9_007_199_254_740_991;

mod human_u64 {
    use alloc::{
        format,
        string::{String, ToString},
    };
    use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

    use super::MAXIMUM_EXACT_JAVASCRIPT_INTEGER;

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() && *value > MAXIMUM_EXACT_JAVASCRIPT_INTEGER {
            serializer.serialize_str(&value.to_string())
        } else {
            serializer.serialize_u64(*value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return u64::deserialize(deserializer);
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum HumanInteger {
            Number(u64),
            Decimal(String),
        }
        match HumanInteger::deserialize(deserializer)? {
            HumanInteger::Number(value) if value <= MAXIMUM_EXACT_JAVASCRIPT_INTEGER => Ok(value),
            HumanInteger::Number(value) => Err(D::Error::custom(format!(
                "JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"
            ))),
            HumanInteger::Decimal(value) => value
                .parse()
                .map_err(|_| D::Error::custom("invalid exact u64 decimal string")),
        }
    }
}

mod human_i64 {
    use alloc::{
        format,
        string::{String, ToString},
    };
    use serde::{de::Error as _, Deserialize, Deserializer, Serializer};

    use super::MAXIMUM_EXACT_JAVASCRIPT_INTEGER;

    const MINIMUM_EXACT_JAVASCRIPT_INTEGER: i64 = -(MAXIMUM_EXACT_JAVASCRIPT_INTEGER as i64);
    const MAXIMUM_EXACT_JAVASCRIPT_SIGNED_INTEGER: i64 = MAXIMUM_EXACT_JAVASCRIPT_INTEGER as i64;

    pub fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable()
            && !(MINIMUM_EXACT_JAVASCRIPT_INTEGER..=MAXIMUM_EXACT_JAVASCRIPT_SIGNED_INTEGER)
                .contains(value)
        {
            serializer.serialize_str(&value.to_string())
        } else {
            serializer.serialize_i64(*value)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return i64::deserialize(deserializer);
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum HumanInteger {
            Number(i64),
            Decimal(String),
        }
        match HumanInteger::deserialize(deserializer)? {
            HumanInteger::Number(value)
                if (MINIMUM_EXACT_JAVASCRIPT_INTEGER..=MAXIMUM_EXACT_JAVASCRIPT_SIGNED_INTEGER)
                    .contains(&value) =>
            {
                Ok(value)
            }
            HumanInteger::Number(value) => Err(D::Error::custom(format!(
                "JSON integer {value} exceeds JavaScript's exact integer range; encode it as a decimal string"
            ))),
            HumanInteger::Decimal(value) => value
                .parse()
                .map_err(|_| D::Error::custom("invalid exact i64 decimal string")),
        }
    }
}

/// A machine-readable semantic law owned by a Kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KindSemanticLaw {
    Terminal(KindTerminalBehavior),
    /// How this Kind transforms terminal truth. This is deliberately
    /// independent of every Port's value, temporal, and abnormal-info types.
    TerminalTransduction(TerminalTransductionProfile),
    ExternalEffects(ExternalEffectBehavior),
    TemporalState(TemporalStateBehavior),
    TimeDependence(SemanticDependence),
    RandomDependence(SemanticDependence),
    ResourceDependence(SemanticDependence),
    Suspension(SuspensionBehavior),
    Variability(VariabilityBehavior),
    Replay(ReplayBehavior),
    /// Exact Fore ports whose values are unforgeable resource authority rather
    /// than serializable info.
    ResourcePorts(Vec<crate::ResourcePortContract>),
    /// Exact finite contracts for values at this Kind's Fore.
    ValueContracts(Vec<crate::FrontValueContract>),
    /// Finite correlation semantics for a two-sided one-to-one keyed join.
    KeyedJoin(KeyedJoinSemanticLaw),
    /// Finite collection of one closing Flow into exactly one sequence Value.
    BoundedCollect(BoundedCollectSemanticLaw),
    /// Exact bounded lifting of one Value transform over a closing Flow.
    FlowEach(FlowEachSemanticLaw),
    /// Exact bounded lifting of one Value-to-Boolean predicate over a closing Flow.
    FlowSelect(FlowSelectSemanticLaw),
    /// Exact bounded left fold of one closing Flow through a reviewed combine Plot.
    FlowFold(FlowFoldSemanticLaw),
    /// Exact bounded retained progression through a reviewed combine Plot.
    FlowScan(FlowScanSemanticLaw),
}

pub use crate::flow_semantic_laws::*;

/// Independent terminal behaviors owned by a Kind's checked Fore.
///
/// This is not a Port modality and does not describe the type carried by `!`.
/// It records what an implementation must do after receiving close, abnormal,
/// or cancellation truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalTransductionProfile {
    /// Exact input Fore port whose terminal truth this contract consumes.
    pub input_port_id: PortId,
    /// Exact output Fore port on which propagated terminal truth appears.
    pub output_port_id: PortId,
    pub normal_close: NormalCloseTransduction,
    pub abnormal: AbnormalTerminalTransduction,
    pub cancellation: CancellationTransduction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalCloseTransduction {
    NotAccepted,
    PropagateAfterDrain,
    Consume,
    FlushThenPropagate(FiniteTerminalEmission),
    /// Consume each input close independently and propagate output closure
    /// only after every contracted input has closed. Closure owes no output.
    PropagateWhenAllClose,
    /// Consume each input close independently, retaining the Gear until every
    /// contracted input has closed. The last close propagates output closure;
    /// each close may flush only the stated finite already-owed output.
    FlushThenPropagateWhenAllClose(FiniteTerminalEmission),
    DomainSpecific {
        law: KindId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbnormalTerminalTransduction {
    NotAccepted,
    PropagateAfterDrain,
    Recover,
    FinalizeThenPropagate(FiniteTerminalEmission),
    DomainSpecific { law: KindId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationTransduction {
    NotCancellable,
    /// A request may be accepted, denied, or stale. Acceptance eventually
    /// produces the exact typed abnormal disposition named here.
    Request {
        disposition_kind: KindId,
    },
    DomainSpecific {
        law: KindId,
    },
}

/// Exact additional output work permitted only by a terminal behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiniteTerminalEmission {
    pub maximum_items: u16,
    pub maximum_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalEffectBehavior {
    None,
    Observable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalStateBehavior {
    None,
    Retained,
    /// One explicit seed, a delayed replacement Flow, and committed Current.
    SourceSeededFinite,
    /// One closing observation Flow containing exactly one value per committed generation.
    SourceSeededFlowFinite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticDependence {
    None,
    ExplicitInput,
    Ambient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuspensionBehavior {
    Never,
    MaySuspend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariabilityBehavior {
    DeterministicFromInputs,
    AdmittedVariability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayBehavior {
    Exact,
    Ineligible,
    Idempotent { operation_key_kind: KindId },
    Transactional { transaction_contract: KindId },
    Compensatable { compensation_contract: KindId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KindTerminalBehavior {
    EmitsOnce,
    EmitsOnceWhenScopeIsEligible,
    CompletesAfterConfiguredCount,
    CompletesAfterFixedCount {
        count: u64,
    },
    CompletesWhenInputsClose,
    MirrorsInputTerminal,
    RetainsLatestUntilReleased,
    EmitsCurrentAndCompletesWhenInputCloses,
    CoupledAtomicFanoutAndMirrorsInputTerminal,
    FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner,
    CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose,
    CurrentScalarSelectorCompletesWhenInputsClose,
    EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
    TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
    InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
    DelaysEachValueInOrderAndDrainsOnInputClosure,
    LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
    SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses,
    /// Retain at most the stated number of values after the first arrival,
    /// emit them in admission order when the local monotonic duration ends,
    /// and let the boundary win an exactly simultaneous arrival tie.
    TumblingProcessingTimeWindow {
        maximum_items: u16,
    },
    SimulatedCurrentObservationEmitsOnce,
    HostInputEndsOrFailsSource,
    HostObservationEndsOrFailsSource,
    EmitsInitialAndTogglesUntilInputCloses,
    EmitsOneField,
    EvolvesAfterTicksAndCompletesWhenTickCloses,
    PresentsEachFieldAndCompletesWhenInputCloses,
    CompletesAfterDockedRefusedOrDeadline,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_count, kind_id, StructuredInfoType, StructuredInfoValue};
    use alloc::vec;

    #[test]
    fn structured_configuration_requires_matching_profile_and_canonical_value() {
        let value_type = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
        let value =
            StructuredInfoValue::leaf(value_type.clone(), encode_count(7).to_vec()).unwrap();
        let canonical = value.canonical_bytes().unwrap();
        let profile = value_type.profile().unwrap().value_kind().clone();

        assert!(StructuredConfigurationValue::new(profile, canonical.clone()).is_some());
        assert!(
            StructuredConfigurationValue::new(kind_id("structured-info/wrong@1"), canonical)
                .is_none()
        );
        assert!(StructuredConfigurationValue::new(
            value_type.profile().unwrap().value_kind().clone(),
            vec![0xff],
        )
        .is_none());
    }
}
