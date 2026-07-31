//! Host-neutral structural-flow contracts.

use core::fmt;

use crate::{Id, PinnedDescriptor, TypeContractRef};

/// Whether branch backpressure is shared or owned by an explicit duplicator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanOutMode {
    Coupled,
    Isolated,
}

impl FanOutMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Coupled => "coupled",
            Self::Isolated => "isolated",
        }
    }
}

/// Exact rule authorizing one logical value to reach several branches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicationRule<'a> {
    /// The representation is a profile-pinned shared handle.
    SharedHandle,
    /// A domain/provider-owned copy relation is selected explicitly.
    Copy(PinnedDescriptor<'a>),
}

/// Portable deterministic merge ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeOrdering<'a> {
    /// Queue arrival sequence, then declared input ordinal for ties.
    Arrival,
    /// One available value per input in ascending ordinal cycles.
    RoundRobin,
    /// Lowest numeric priority first; ordinal breaks ties. The finite turn
    /// bound forces an available lower-priority input to run.
    Priority { starvation_turns: u32 },
    /// Minimum event time no later than the explicit watermark.
    EventTime {
        timestamp_type: TypeContractRef<'a>,
        maximum_lateness_ticks: u64,
        late_values: LateValuePolicy,
    },
}

impl MergeOrdering<'_> {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Arrival => "arrival",
            Self::RoundRobin => "round-robin",
            Self::Priority { .. } => "priority",
            Self::EventTime { .. } => "event-time",
        }
    }
}

/// Exact handling after an event-time watermark makes a value late.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LateValuePolicy {
    Reject,
    DropDisposable,
    Fail,
}

impl LateValuePolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reject => "reject",
            Self::DropDisposable => "drop-disposable",
            Self::Fail => "fail",
        }
    }
}

/// Merge completion behavior after input terminals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeTerminalPolicy {
    /// Drain every input and complete after all inputs complete.
    DrainAll,
    /// Complete after the first input terminal and apply the exact run/cord
    /// cancellation policy to the remainder.
    CompleteAny,
    /// Fail if any input fails; otherwise drain all successful inputs.
    FailFastDrainSuccess,
}

/// One currently available merge input. Hosted runtimes may attach storage to
/// the ordinal, but portable selection depends only on these bounded facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeCandidate {
    pub input_ordinal: u16,
    pub arrival_sequence: u64,
    pub priority: u16,
    pub waited_turns: u32,
    pub event_tick: Option<u64>,
}

/// Result of one deterministic merge selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeSelection {
    Ready {
        input_ordinal: u16,
    },
    Late {
        input_ordinal: u16,
        policy: LateValuePolicy,
    },
    WaitingForWatermark,
    Empty,
}

/// Choose one available input without consulting task wake order, wall clock,
/// registry order, or allocation order.
#[must_use]
pub fn select_merge(
    ordering: MergeOrdering<'_>,
    candidates: &[MergeCandidate],
    round_robin_cursor: u16,
    watermark_tick: Option<u64>,
) -> MergeSelection {
    if candidates.is_empty() {
        return MergeSelection::Empty;
    }
    let selected = match ordering {
        MergeOrdering::Arrival => candidates
            .iter()
            .min_by_key(|candidate| (candidate.arrival_sequence, candidate.input_ordinal)),
        MergeOrdering::RoundRobin => candidates
            .iter()
            .filter(|candidate| candidate.input_ordinal >= round_robin_cursor)
            .min_by_key(|candidate| candidate.input_ordinal)
            .or_else(|| {
                candidates
                    .iter()
                    .min_by_key(|candidate| candidate.input_ordinal)
            }),
        MergeOrdering::Priority { starvation_turns } => candidates
            .iter()
            .filter(|candidate| candidate.waited_turns >= starvation_turns)
            .max_by_key(|candidate| (candidate.waited_turns, u16::MAX - candidate.input_ordinal))
            .or_else(|| {
                candidates
                    .iter()
                    .min_by_key(|candidate| (candidate.priority, candidate.input_ordinal))
            }),
        MergeOrdering::EventTime {
            maximum_lateness_ticks,
            late_values,
            ..
        } => {
            let Some(watermark) = watermark_tick else {
                return MergeSelection::WaitingForWatermark;
            };
            let selected = candidates.iter().min_by_key(|candidate| {
                (
                    candidate.event_tick.unwrap_or(u64::MAX),
                    candidate.input_ordinal,
                )
            });
            let Some(selected) = selected else {
                return MergeSelection::Empty;
            };
            let Some(event_tick) = selected.event_tick else {
                return MergeSelection::WaitingForWatermark;
            };
            if event_tick.saturating_add(maximum_lateness_ticks) < watermark {
                return MergeSelection::Late {
                    input_ordinal: selected.input_ordinal,
                    policy: late_values,
                };
            }
            if event_tick > watermark {
                return MergeSelection::WaitingForWatermark;
            }
            Some(selected)
        }
    };
    selected.map_or(MergeSelection::Empty, |candidate| MergeSelection::Ready {
        input_ordinal: candidate.input_ordinal,
    })
}

impl MergeTerminalPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DrainAll => "drain-all",
            Self::CompleteAny => "complete-any",
            Self::FailFastDrainSuccess => "fail-fast-drain-success",
        }
    }
}

/// Ordinary structural-node families. These are contracts, never compiler
/// magic or implicit coercions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralNodeKind {
    Identity,
    Duplicator,
    Merge,
    Zip,
    CombineLatest,
    Mux,
    Demux,
    KeyedDispatch,
    Select,
    Gate,
    Switch,
    Fallback,
    FeedbackDelay,
    Buffer,
    Throttle,
    Adapter,
}

/// Bounds shared by ordinary structural node contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralNodeLimits {
    pub maximum_state_values: u16,
    pub maximum_state_bytes: u64,
    pub maximum_pending_inputs: u16,
    pub maximum_outputs_per_step: u16,
}

impl StructuralNodeLimits {
    pub const fn validate(self) -> Result<(), StructuralError> {
        if self.maximum_state_values == 0
            || self.maximum_state_bytes == 0
            || self.maximum_pending_inputs == 0
            || self.maximum_outputs_per_step == 0
        {
            return Err(StructuralError::Unbounded);
        }
        Ok(())
    }
}

/// Exact named adapter selection. Compatibility never inserts it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterContract<'a> {
    pub id: Id<'a>,
    pub input_type: TypeContractRef<'a>,
    pub output_type: TypeContractRef<'a>,
    pub implementation_contract: PinnedDescriptor<'a>,
}

/// Structural contract failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralError {
    InvalidDescriptor,
    Unbounded,
    InvalidTopology,
    DuplicationNotAuthorized,
    InvalidOrdering,
}

impl StructuralError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidDescriptor => "CND-STR-001",
            Self::Unbounded => "CND-STR-002",
            Self::InvalidTopology => "CND-STR-003",
            Self::DuplicationNotAuthorized => "CND-STR-004",
            Self::InvalidOrdering => "CND-STR-005",
        }
    }
}

impl fmt::Display for StructuralError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDescriptor => "structural contract descriptor is invalid",
            Self::Unbounded => "structural node state and work must be finite",
            Self::InvalidTopology => "structural plan topology is inconsistent",
            Self::DuplicationNotAuthorized => "fan-out lacks an explicit safe copy or sharing rule",
            Self::InvalidOrdering => "merge ordering or terminal policy is invalid",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SemanticHash;

    const TYPE: TypeContractRef<'static> = TypeContractRef {
        contract_id: Id("fixture/timestamp"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([7; 32]),
    };

    const fn candidate(
        input_ordinal: u16,
        arrival_sequence: u64,
        priority: u16,
        waited_turns: u32,
        event_tick: u64,
    ) -> MergeCandidate {
        MergeCandidate {
            input_ordinal,
            arrival_sequence,
            priority,
            waited_turns,
            event_tick: Some(event_tick),
        }
    }

    #[test]
    fn merge_selection_is_policy_ordered_and_tie_broken() {
        let candidates = [candidate(1, 3, 2, 0, 12), candidate(0, 3, 2, 0, 10)];
        assert_eq!(
            select_merge(MergeOrdering::Arrival, &candidates, 0, None),
            MergeSelection::Ready { input_ordinal: 0 }
        );
        assert_eq!(
            select_merge(MergeOrdering::RoundRobin, &candidates, 1, None),
            MergeSelection::Ready { input_ordinal: 1 }
        );
        assert_eq!(
            select_merge(
                MergeOrdering::Priority {
                    starvation_turns: 4
                },
                &candidates,
                0,
                None,
            ),
            MergeSelection::Ready { input_ordinal: 0 }
        );
    }

    #[test]
    fn priority_starvation_and_event_time_lateness_are_explicit() {
        let candidates = [candidate(0, 0, 0, 0, 20), candidate(1, 1, 9, 4, 5)];
        assert_eq!(
            select_merge(
                MergeOrdering::Priority {
                    starvation_turns: 4
                },
                &candidates,
                0,
                None,
            ),
            MergeSelection::Ready { input_ordinal: 1 }
        );
        assert_eq!(
            select_merge(
                MergeOrdering::EventTime {
                    timestamp_type: TYPE,
                    maximum_lateness_ticks: 2,
                    late_values: LateValuePolicy::Fail,
                },
                &candidates,
                0,
                Some(10),
            ),
            MergeSelection::Late {
                input_ordinal: 1,
                policy: LateValuePolicy::Fail,
            }
        );
        assert_eq!(
            select_merge(
                MergeOrdering::EventTime {
                    timestamp_type: TYPE,
                    maximum_lateness_ticks: 2,
                    late_values: LateValuePolicy::Reject,
                },
                &[candidate(0, 0, 0, 0, 20)],
                0,
                Some(10),
            ),
            MergeSelection::WaitingForWatermark
        );
    }
}
