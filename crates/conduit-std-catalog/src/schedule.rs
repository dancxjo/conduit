//! Portable schedule intent and workflow observation semantics.
//!
//! Recurrence, civil-time resolution, and clock truth belong to the temporal
//! substrate in `conduit-core`. This module only joins those exact values to a
//! finite scheduled intent and to observed workflow state. It never schedules
//! or executes an effect.

#[cfg(feature = "form-catalog")]
use alloc::{vec, vec::Vec};
#[cfg(feature = "form-catalog")]
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase, QUANTITY_INFO_ID,
};
use conduit_core::{Quantity, QuantityDimension};

#[cfg(feature = "form-catalog")]
use crate::recurrence_occurrence_instant_type;
#[cfg(feature = "form-catalog")]
use crate::recurrence_occurrence_type;

pub const SCHEDULED_INTENT_TYPE: &str = "ScheduledIntent";
pub const SCHEDULE_WORKFLOW_LIFECYCLE_TYPE: &str = "ScheduleWorkflowLifecycle";
pub const SCHEDULE_OBSERVATION_TYPE: &str = "ScheduleObservation";
pub const SCHEDULE_ASSESSMENT_TYPE: &str = "ScheduleAssessment";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkflowLifecycle {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScheduleWindowPosition {
    Before,
    Within,
    After,
    Indeterminate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkflowTimingOutcome {
    Awaiting,
    OnTime,
    Late { lateness: Quantity },
    MissedWindow,
    ClockUncertain { uncertainty: Quantity },
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScheduleRefusal {
    NonTemporalQuantity,
    NegativeQuantity,
    InconsistentLifecycle,
}

pub fn assess_workflow_timing(
    lifecycle: WorkflowLifecycle,
    position: ScheduleWindowPosition,
    offset_from_boundary: Quantity,
    uncertainty: Quantity,
) -> Result<WorkflowTimingOutcome, ScheduleRefusal> {
    validate_duration(offset_from_boundary)?;
    validate_duration(uncertainty)?;
    match lifecycle {
        WorkflowLifecycle::Failed => return Ok(WorkflowTimingOutcome::Failed),
        WorkflowLifecycle::Cancelled => return Ok(WorkflowTimingOutcome::Cancelled),
        WorkflowLifecycle::Expired => return Ok(WorkflowTimingOutcome::Expired),
        WorkflowLifecycle::Pending | WorkflowLifecycle::Running | WorkflowLifecycle::Completed => {}
    }
    if position == ScheduleWindowPosition::Indeterminate || uncertainty.value() > 0 {
        return Ok(WorkflowTimingOutcome::ClockUncertain { uncertainty });
    }
    match (lifecycle, position) {
        (
            WorkflowLifecycle::Pending,
            ScheduleWindowPosition::Before | ScheduleWindowPosition::Within,
        ) => Ok(WorkflowTimingOutcome::Awaiting),
        (WorkflowLifecycle::Pending, ScheduleWindowPosition::After) => {
            Ok(WorkflowTimingOutcome::MissedWindow)
        }
        (
            WorkflowLifecycle::Running | WorkflowLifecycle::Completed,
            ScheduleWindowPosition::Within,
        ) => Ok(WorkflowTimingOutcome::OnTime),
        (
            WorkflowLifecycle::Running | WorkflowLifecycle::Completed,
            ScheduleWindowPosition::After,
        ) => Ok(WorkflowTimingOutcome::Late {
            lateness: offset_from_boundary,
        }),
        (
            WorkflowLifecycle::Running | WorkflowLifecycle::Completed,
            ScheduleWindowPosition::Before,
        ) => Err(ScheduleRefusal::InconsistentLifecycle),
        (_, ScheduleWindowPosition::Indeterminate) => unreachable!("handled above"),
        (
            WorkflowLifecycle::Failed | WorkflowLifecycle::Cancelled | WorkflowLifecycle::Expired,
            _,
        ) => unreachable!("terminal state handled above"),
    }
}

fn validate_duration(value: Quantity) -> Result<(), ScheduleRefusal> {
    if value.dimension() != QuantityDimension::Time {
        return Err(ScheduleRefusal::NonTemporalQuantity);
    }
    if value.value() < 0 {
        return Err(ScheduleRefusal::NegativeQuantity);
    }
    Ok(())
}

#[cfg(feature = "form-catalog")]
fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed schedule leaf")
}

#[cfg(feature = "form-catalog")]
fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed schedule field")
}

#[cfg(feature = "form-catalog")]
fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed schedule case")
}

#[cfg(feature = "form-catalog")]
fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed schedule record")
}

#[cfg(feature = "form-catalog")]
fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

#[cfg(feature = "form-catalog")]
fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

#[cfg(feature = "form-catalog")]
pub fn schedule_effect_intent_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("schedule/effect-intent@1"),
        vec![
            case("observation", text_type()),
            case("proposed_effect", text_type()),
        ],
    )
    .expect("reviewed effect proposal")
}

#[cfg(feature = "form-catalog")]
pub fn schedule_constraint_type() -> StructuredInfoType {
    let instant = recurrence_occurrence_instant_type();
    let window = record(
        "schedule/temporal-window@1",
        vec![
            field("end", instant.clone()),
            field("start", instant.clone()),
        ],
    );
    StructuredInfoType::variant(
        kind_id("schedule/temporal-constraint@1"),
        vec![case("deadline", instant), case("window", window)],
    )
    .expect("reviewed schedule constraint")
}

#[cfg(feature = "form-catalog")]
pub fn scheduled_intent_type() -> StructuredInfoType {
    record(
        "schedule/scheduled-intent@1",
        vec![
            field("constraint", schedule_constraint_type()),
            field("effect", schedule_effect_intent_type()),
            field("identity", text_type()),
            field("occurrence", recurrence_occurrence_type()),
        ],
    )
}

#[cfg(feature = "form-catalog")]
pub fn workflow_lifecycle_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("schedule/workflow-lifecycle@1"),
        vec![
            case("cancelled", unit_type()),
            case("completed", unit_type()),
            case("expired", unit_type()),
            case("failed", text_type()),
            case("pending", unit_type()),
            case("running", unit_type()),
        ],
    )
    .expect("reviewed workflow lifecycle")
}

#[cfg(feature = "form-catalog")]
pub fn schedule_window_position_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("schedule/window-position@1"),
        vec![
            case("after", unit_type()),
            case("before", unit_type()),
            case("indeterminate", unit_type()),
            case("within", unit_type()),
        ],
    )
    .expect("reviewed schedule position")
}

#[cfg(feature = "form-catalog")]
pub fn schedule_observation_type() -> StructuredInfoType {
    record(
        "schedule/observation@1",
        vec![
            field("observed_at", recurrence_occurrence_instant_type()),
            field("offset_from_boundary", leaf(QUANTITY_INFO_ID)),
            field("position", schedule_window_position_type()),
            field("uncertainty", leaf(QUANTITY_INFO_ID)),
        ],
    )
}

#[cfg(feature = "form-catalog")]
pub fn workflow_timing_outcome_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("schedule/workflow-timing-outcome@1"),
        vec![
            case("awaiting", unit_type()),
            case("cancelled", unit_type()),
            case("clock_uncertain", leaf(QUANTITY_INFO_ID)),
            case("expired", unit_type()),
            case("failed", unit_type()),
            case("late", leaf(QUANTITY_INFO_ID)),
            case("missed_window", unit_type()),
            case("on_time", unit_type()),
        ],
    )
    .expect("reviewed workflow timing outcome")
}

#[cfg(feature = "form-catalog")]
pub fn schedule_assessment_type() -> StructuredInfoType {
    record(
        "schedule/assessment@1",
        vec![
            field("intent_identity", text_type()),
            field("outcome", workflow_timing_outcome_type()),
        ],
    )
}

#[cfg(feature = "form-catalog")]
pub fn schedule_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (SCHEDULED_INTENT_TYPE, scheduled_intent_type()),
        (SCHEDULE_WORKFLOW_LIFECYCLE_TYPE, workflow_lifecycle_type()),
        (SCHEDULE_OBSERVATION_TYPE, schedule_observation_type()),
        (SCHEDULE_ASSESSMENT_TYPE, schedule_assessment_type()),
    ]
}
