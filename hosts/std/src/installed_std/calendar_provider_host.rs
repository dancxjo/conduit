//! Pre-Play request preparation and bounded calendar host dispatch.

use conduit_core::{
    kind_id, PlanFragment, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape,
};

use super::calendar_provider_operation;
use crate::hosted_calendar::{
    CalendarHostedOperation, GoogleCalendarRefusal, HostedCalendarAdapter,
};

struct PreparedNode {
    operation: CalendarHostedOperation,
    request_canonical: Vec<u8>,
    semantic_json: Vec<u8>,
}

pub(super) struct CalendarProviderHost {
    nodes: Vec<Option<PreparedNode>>,
}

impl CalendarProviderHost {
    pub(super) fn prepare(fragment: &PlanFragment) -> Result<Self, String> {
        let nodes = fragment
            .placements
            .iter()
            .map(|placement| {
                let Some(operation) = calendar_provider_operation::operation(placement) else {
                    return Ok(None);
                };
                let request = calendar_provider_operation::request_value(placement)?;
                let request_canonical = request
                    .canonical_bytes()
                    .map_err(|error| format!("encode prepared calendar request: {error:?}"))?;
                let semantic_json = envelope_text(&request, "semantic_json")
                    .map_err(|error| format!("decode calendar semantic request: {error:?}"))?
                    .to_vec();
                if semantic_json.len()
                    > conduit_semantic_catalog::CALENDAR_MAXIMUM_SEMANTIC_JSON_BYTES as usize
                {
                    return Err("calendar semantic request exceeds the admitted bound".into());
                }
                Ok(Some(PreparedNode {
                    operation,
                    request_canonical,
                    semantic_json,
                }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { nodes })
    }

    pub(super) fn execute(
        &mut self,
        node: usize,
        operation: CalendarHostedOperation,
        input: &[u8],
        adapter: Option<&mut (dyn HostedCalendarAdapter + 'static)>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let prepared = self
            .nodes
            .get(node)
            .and_then(Option::as_ref)
            .ok_or(GoogleCalendarRefusal::InvalidRequest)?;
        if prepared.operation != operation {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        let prior = if matches!(
            operation,
            CalendarHostedOperation::Update
                | CalendarHostedOperation::Cancel
                | CalendarHostedOperation::Invite
        ) {
            Some(realization_json(input)?)
        } else {
            if input != prepared.request_canonical {
                return Err(GoogleCalendarRefusal::InvalidRequest);
            }
            None
        };
        let adapter = adapter.ok_or(GoogleCalendarRefusal::ProviderLost)?;
        let realization = adapter.execute(operation, &prepared.semantic_json, prior.as_deref())?;
        encode_result(operation, realization)
    }
}

pub(super) const fn refusal_detail(refusal: GoogleCalendarRefusal) -> u16 {
    match refusal {
        GoogleCalendarRefusal::InvalidCredential => 1,
        GoogleCalendarRefusal::InvalidResource => 2,
        GoogleCalendarRefusal::InvalidRequest => 3,
        GoogleCalendarRefusal::Capacity => 4,
        GoogleCalendarRefusal::AuthorityDenied => 5,
        GoogleCalendarRefusal::StaleRevision => 6,
        GoogleCalendarRefusal::CalendarDeleted => 7,
        GoogleCalendarRefusal::EventDeleted => 8,
        GoogleCalendarRefusal::AttendeeWriteDenied => 9,
        GoogleCalendarRefusal::RateLimited => 10,
        GoogleCalendarRefusal::ProviderLost => 11,
        GoogleCalendarRefusal::ProviderResponseMalformed => 12,
        GoogleCalendarRefusal::ProviderResponseTooLarge => 13,
        GoogleCalendarRefusal::TimezoneMismatch => 14,
        GoogleCalendarRefusal::RecurrenceMismatch => 15,
        GoogleCalendarRefusal::StaleFreeBusy => 16,
    }
}

fn realization_json(input: &[u8]) -> Result<Vec<u8>, GoogleCalendarRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|_| GoogleCalendarRefusal::InvalidRequest)?;
    if value.value_type() != &conduit_semantic_catalog::calendar_write_receipt_type() {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    }
    Ok(envelope_text(&value, "realization_json")?.to_vec())
}

fn encode_result(
    operation: CalendarHostedOperation,
    realization: Vec<u8>,
) -> Result<Vec<u8>, GoogleCalendarRefusal> {
    if realization.len() > conduit_semantic_catalog::CALENDAR_MAXIMUM_RESULT_BYTES as usize
        || core::str::from_utf8(&realization).is_err()
    {
        return Err(GoogleCalendarRefusal::ProviderResponseTooLarge);
    }
    let value_type = match operation {
        CalendarHostedOperation::Read => conduit_semantic_catalog::calendar_read_result_type(),
        CalendarHostedOperation::FreeBusy => {
            conduit_semantic_catalog::calendar_free_busy_result_type()
        }
        CalendarHostedOperation::Create
        | CalendarHostedOperation::Update
        | CalendarHostedOperation::Invite => {
            conduit_semantic_catalog::calendar_write_receipt_type()
        }
        CalendarHostedOperation::Cancel => conduit_semantic_catalog::calendar_cancel_receipt_type(),
    };
    let leaf = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/text@1"))
            .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?,
        realization,
    )
    .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    let value = StructuredInfoValue::record(
        value_type,
        vec![StructuredFieldValue::new("realization_json", leaf)
            .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?],
    )
    .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    value
        .canonical_bytes()
        .map_err(|_| GoogleCalendarRefusal::ProviderResponseTooLarge)
}

fn envelope_text<'a>(
    value: &'a StructuredInfoValue,
    field_name: &str,
) -> Result<&'a [u8], GoogleCalendarRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    };
    let field = fields
        .iter()
        .find(|field| field.name() == field_name)
        .map(StructuredFieldValue::value)
        .ok_or(GoogleCalendarRefusal::InvalidRequest)?;
    let StructuredInfoValueShape::Leaf(bytes) = field.shape() else {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    };
    core::str::from_utf8(bytes).map_err(|_| GoogleCalendarRefusal::InvalidRequest)?;
    Ok(bytes)
}
