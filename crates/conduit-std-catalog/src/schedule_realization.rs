//! Deterministic fixtures and pure workflow assessment over schedule Info.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityUnit, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};

use crate::{
    assess_workflow_timing, recurrence_instant_type, recurrence_occurrence_instant_type,
    recurrence_occurrence_type, schedule_assessment_type, schedule_constraint_type,
    schedule_effect_intent_type, schedule_observation_type, schedule_window_position_type,
    scheduled_intent_type, workflow_lifecycle_type, workflow_timing_outcome_type, ScheduleRefusal,
    ScheduleWindowPosition, WorkflowLifecycle, WorkflowTimingOutcome,
};

pub struct ScheduleFixture {
    pub intent: StructuredInfoValue,
    pub lifecycle: StructuredInfoValue,
    pub observation: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleInfoRefusal {
    MalformedInfo,
    Semantic(ScheduleRefusal),
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for ScheduleInfoRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<ScheduleRefusal> for ScheduleInfoRefusal {
    fn from(value: ScheduleRefusal) -> Self {
        Self::Semantic(value)
    }
}

pub fn deterministic_schedule_fixture() -> Result<ScheduleFixture, ScheduleInfoRefusal> {
    let start = wall_occurrence_instant(100)?;
    let end = wall_occurrence_instant(200)?;
    let observed = wall_occurrence_instant(202)?;
    let constraint_type = schedule_constraint_type();
    let window_type = variant_payload_type(&constraint_type, "window")?;
    let constraint = StructuredInfoValue::variant(
        constraint_type,
        "window",
        record_value(window_type, vec![("end", end), ("start", start.clone())])?,
    )?;
    let effect = StructuredInfoValue::variant(
        schedule_effect_intent_type(),
        "proposed_effect",
        text_value("job/report"),
    )?;
    let occurrence = record_value(
        recurrence_occurrence_type(),
        vec![
            ("identity", text_value("recurrence/report#0")),
            ("instant", start),
            ("ordinal", count_value(0)),
            ("recurrence_identity", text_value("recurrence/report")),
        ],
    )?;
    let intent = record_value(
        scheduled_intent_type(),
        vec![
            ("constraint", constraint),
            ("effect", effect),
            ("identity", text_value("schedule/report#0")),
            ("occurrence", occurrence),
        ],
    )?;
    let lifecycle = unit_variant(workflow_lifecycle_type(), "completed")?;
    let observation = record_value(
        schedule_observation_type(),
        vec![
            ("observed_at", observed),
            (
                "offset_from_boundary",
                quantity_value(2, QuantityUnit::Second)?,
            ),
            (
                "position",
                unit_variant(schedule_window_position_type(), "after")?,
            ),
            ("uncertainty", quantity_value(0, QuantityUnit::Second)?),
        ],
    )?;
    Ok(ScheduleFixture {
        intent,
        lifecycle,
        observation,
    })
}

pub fn assess_schedule_values(
    intent: &StructuredInfoValue,
    lifecycle: &StructuredInfoValue,
    observation: &StructuredInfoValue,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    if intent.value_type() != &scheduled_intent_type()
        || lifecycle.value_type() != &workflow_lifecycle_type()
        || observation.value_type() != &schedule_observation_type()
    {
        return Err(ScheduleInfoRefusal::MalformedInfo);
    }
    let identity = leaf_text(record_field(intent, "identity")?)?;
    let lifecycle = decode_lifecycle(lifecycle)?;
    let position = decode_position(record_field(observation, "position")?)?;
    let offset = decode_quantity(record_field(observation, "offset_from_boundary")?)?;
    let uncertainty = decode_quantity(record_field(observation, "uncertainty")?)?;
    let outcome = assess_workflow_timing(lifecycle, position, offset, uncertainty)?;
    record_value(
        schedule_assessment_type(),
        vec![
            ("intent_identity", text_value(identity)),
            ("outcome", outcome_value(outcome)?),
        ],
    )
}

pub fn workflow_lifecycle_value(
    lifecycle: WorkflowLifecycle,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    let tag = match lifecycle {
        WorkflowLifecycle::Pending => "pending",
        WorkflowLifecycle::Running => "running",
        WorkflowLifecycle::Completed => "completed",
        WorkflowLifecycle::Failed => {
            return Ok(StructuredInfoValue::variant(
                workflow_lifecycle_type(),
                "failed",
                text_value("deterministic failure"),
            )?)
        }
        WorkflowLifecycle::Cancelled => "cancelled",
        WorkflowLifecycle::Expired => "expired",
    };
    unit_variant(workflow_lifecycle_type(), tag)
}

fn decode_lifecycle(value: &StructuredInfoValue) -> Result<WorkflowLifecycle, ScheduleInfoRefusal> {
    match variant_tag(value)? {
        "pending" => Ok(WorkflowLifecycle::Pending),
        "running" => Ok(WorkflowLifecycle::Running),
        "completed" => Ok(WorkflowLifecycle::Completed),
        "failed" => Ok(WorkflowLifecycle::Failed),
        "cancelled" => Ok(WorkflowLifecycle::Cancelled),
        "expired" => Ok(WorkflowLifecycle::Expired),
        _ => Err(ScheduleInfoRefusal::MalformedInfo),
    }
}

fn decode_position(
    value: &StructuredInfoValue,
) -> Result<ScheduleWindowPosition, ScheduleInfoRefusal> {
    match variant_tag(value)? {
        "before" => Ok(ScheduleWindowPosition::Before),
        "within" => Ok(ScheduleWindowPosition::Within),
        "after" => Ok(ScheduleWindowPosition::After),
        "indeterminate" => Ok(ScheduleWindowPosition::Indeterminate),
        _ => Err(ScheduleInfoRefusal::MalformedInfo),
    }
}

fn outcome_value(
    outcome: WorkflowTimingOutcome,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    let value_type = workflow_timing_outcome_type();
    let (tag, payload) = match outcome {
        WorkflowTimingOutcome::Awaiting => ("awaiting", unit_value()?),
        WorkflowTimingOutcome::OnTime => ("on_time", unit_value()?),
        WorkflowTimingOutcome::Late { lateness } => ("late", quantity(lateness)?),
        WorkflowTimingOutcome::MissedWindow => ("missed_window", unit_value()?),
        WorkflowTimingOutcome::ClockUncertain { uncertainty } => {
            ("clock_uncertain", quantity(uncertainty)?)
        }
        WorkflowTimingOutcome::Failed => ("failed", unit_value()?),
        WorkflowTimingOutcome::Cancelled => ("cancelled", unit_value()?),
        WorkflowTimingOutcome::Expired => ("expired", unit_value()?),
    };
    Ok(StructuredInfoValue::variant(value_type, tag, payload)?)
}

fn wall_occurrence_instant(ticks: u64) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    Ok(StructuredInfoValue::variant(
        recurrence_occurrence_instant_type(),
        "wall",
        record_value(
            recurrence_instant_type(),
            vec![
                ("basis", text_value("unix/utc@1")),
                ("resolution_ticks", count_value(1)),
                ("scale", leaf_value("time/scale@1", b"seconds".to_vec())?),
                ("ticks", count_value(ticks)),
            ],
        )?,
    )?)
}

fn quantity_value(
    value: i64,
    unit: QuantityUnit,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    quantity(Quantity::new(value, unit))
}

fn quantity(value: Quantity) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    leaf_value(conduit_core::QUANTITY_INFO_ID, value.encode().to_vec())
}

fn decode_quantity(value: &StructuredInfoValue) -> Result<Quantity, ScheduleInfoRefusal> {
    Quantity::decode(leaf_bytes(value)?).map_err(|_| ScheduleInfoRefusal::MalformedInfo)
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        unit_value()?,
    )?)
}

fn unit_value() -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    leaf_value("value/unit@1", Vec::new())
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded deterministic text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded deterministic count")
}

fn leaf_value(kind: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, ScheduleInfoRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, ScheduleInfoRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(ScheduleInfoRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(ScheduleInfoRefusal::MalformedInfo)
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, ScheduleInfoRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(ScheduleInfoRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(ScheduleInfoRefusal::MalformedInfo)
}

fn variant_tag(value: &StructuredInfoValue) -> Result<&str, ScheduleInfoRefusal> {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        return Err(ScheduleInfoRefusal::MalformedInfo);
    };
    Ok(tag)
}

fn leaf_text(value: &StructuredInfoValue) -> Result<&str, ScheduleInfoRefusal> {
    core::str::from_utf8(leaf_bytes(value)?).map_err(|_| ScheduleInfoRefusal::MalformedInfo)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], ScheduleInfoRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(ScheduleInfoRefusal::MalformedInfo);
    };
    Ok(bytes)
}
