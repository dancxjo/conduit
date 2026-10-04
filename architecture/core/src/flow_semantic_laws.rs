//! Exact finite Flow transformation laws, independent of configuration encoding.
use crate::{KindId, PortId};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowScanSemanticLaw {
    pub input_port_id: PortId,
    pub output_port_id: PortId,
    pub item: crate::CheckedValueContract,
    pub accumulator: crate::CheckedValueContract,
    pub initial_accumulator: Vec<u8>,
    pub combine_accumulator_port_id: PortId,
    pub combine_item_port_id: PortId,
    pub combine_output_port_id: PortId,
    pub maximum_active: u16,
    pub maximum_queued: u16,
    pub maximum_items: u16,
    pub invocation: FlowScanInvocation,
    pub progression: FlowScanProgression,
    pub empty: FlowScanEmptyDisposition,
    pub close: FlowScanCloseDisposition,
    pub abnormal: FlowScanAbnormalDisposition,
    pub cancellation: FlowScanCancellationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanInvocation {
    OncePerAcceptedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanProgression {
    EmitCombinedAccumulatorExactlyOnceInInputOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanEmptyDisposition {
    EmitNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanCloseDisposition {
    DrainThenCloseWithoutExtraEmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanAbnormalDisposition {
    DiscardAccumulatorAndPropagateExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowScanCancellationDisposition {
    DiscardAccumulatorWithoutEmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFoldSemanticLaw {
    pub input_port_id: PortId,
    pub output_port_id: PortId,
    pub item: crate::CheckedValueContract,
    pub accumulator: crate::CheckedValueContract,
    /// Exact canonical accumulator value retained before the first item.
    pub initial_accumulator: Vec<u8>,
    pub combine_accumulator_port_id: PortId,
    pub combine_item_port_id: PortId,
    pub combine_output_port_id: PortId,
    pub maximum_active: u16,
    pub maximum_queued: u16,
    pub maximum_items: u16,
    pub invocation: FlowFoldInvocation,
    pub close: FlowFoldCloseDisposition,
    pub abnormal: FlowFoldAbnormalDisposition,
    pub cancellation: FlowFoldCancellationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEachSemanticLaw {
    pub input_port_id: PortId,
    pub output_port_id: PortId,
    pub maximum_items: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowFoldInvocation {
    OncePerAcceptedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowFoldCloseDisposition {
    DrainThenEmitAccumulatorExactlyOnce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowFoldAbnormalDisposition {
    DiscardAccumulatorAndPropagateExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowFoldCancellationDisposition {
    DiscardAccumulatorWithoutEmission,
}

/// Exact portable law for collecting one closing Flow.
///
/// Normal input close owes exactly one output Value, including for an empty
/// input. Observing more than `maximum_items` produces the exact typed
/// abnormal disposition and never a partial collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedCollectSemanticLaw {
    pub input_port_id: PortId,
    pub output_port_id: PortId,
    pub element: crate::CheckedValueContract,
    pub collection: crate::CheckedValueContract,
    pub maximum_items: u16,
    pub overflow_disposition: crate::CheckedValueContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSelectSemanticLaw {
    pub input_port_id: PortId,
    pub output_port_id: PortId,
    pub predicate_input_kind: KindId,
    pub predicate_output_kind: KindId,
    pub maximum_active: u16,
    pub maximum_queued: u16,
    pub maximum_items: u16,
    pub invocation: FlowSelectInvocation,
    pub retained_input: FlowSelectRetainedInput,
    pub true_disposition: FlowSelectTrueDisposition,
    pub false_disposition: FlowSelectFalseDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSelectInvocation {
    OncePerAcceptedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSelectRetainedInput {
    UntilPredicateCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSelectFalseDisposition {
    EmitNothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowSelectTrueDisposition {
    EmitRetainedInputExactlyOnce,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyedJoinSemanticLaw {
    pub key: crate::CheckedValueContract,
    pub left_value: crate::CheckedValueContract,
    pub right_value: crate::CheckedValueContract,
    pub maximum_pending_per_side: u16,
    pub pairing: KeyedJoinPairing,
    pub output_order: KeyedJoinOutputOrder,
    pub capacity: KeyedJoinCapacityBehavior,
    pub unmatched_on_close: KeyedJoinUnmatchedCloseBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyedJoinPairing {
    OldestWithOldest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyedJoinOutputOrder {
    MatchCompletionArrival,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyedJoinCapacityBehavior {
    BackpressureUnmatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyedJoinUnmatchedCloseBehavior {
    DiscardWhenMatchBecomesImpossible,
}
