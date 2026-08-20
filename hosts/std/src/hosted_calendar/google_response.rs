//! Bounded decoding of provider observations and exact write receipts.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{
    google_protocol::validate_event, GoogleCalendarRefusal, GoogleWireBusyInterval,
    GoogleWireCalendarBusy, GoogleWireEvent, GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES,
    GOOGLE_CALENDAR_MAXIMUM_EVENTS, GOOGLE_CALENDAR_MAXIMUM_ID_BYTES,
    GOOGLE_CALENDAR_MAXIMUM_PAGE_TOKEN_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleEventPage {
    pub events: Vec<GoogleWireEvent>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleFreeBusyPage {
    pub calendars: BTreeMap<String, Vec<GoogleWireBusyInterval>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleWriteReceipt {
    pub event_id: String,
    pub revision: String,
    pub event: GoogleWireEvent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventPageEnvelope {
    #[serde(default)]
    items: Vec<GoogleWireEvent>,
    #[serde(default)]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct FreeBusyEnvelope {
    #[serde(default)]
    calendars: BTreeMap<String, GoogleWireCalendarBusy>,
}

pub(super) fn decode_event_page(
    body: &[u8],
    maximum_results: u16,
) -> Result<GoogleEventPage, GoogleCalendarRefusal> {
    bounded(body)?;
    let page: EventPageEnvelope = serde_json::from_slice(body)
        .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    if page.items.len() > usize::from(maximum_results)
        || page.items.len() > GOOGLE_CALENDAR_MAXIMUM_EVENTS
        || page
            .items
            .iter()
            .any(|event| validate_event(event, true).is_err())
        || page.next_page_token.as_ref().is_some_and(|token| {
            token.is_empty() || token.len() > GOOGLE_CALENDAR_MAXIMUM_PAGE_TOKEN_BYTES
        })
    {
        return Err(GoogleCalendarRefusal::ProviderResponseMalformed);
    }
    Ok(GoogleEventPage {
        events: page.items,
        next_page_token: page.next_page_token,
    })
}

pub(super) fn decode_free_busy(
    body: &[u8],
    requested_calendars: &[String],
) -> Result<GoogleFreeBusyPage, GoogleCalendarRefusal> {
    bounded(body)?;
    let response: FreeBusyEnvelope = serde_json::from_slice(body)
        .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    if response.calendars.len() > requested_calendars.len()
        || response
            .calendars
            .keys()
            .any(|calendar| !requested_calendars.contains(calendar))
    {
        return Err(GoogleCalendarRefusal::ProviderResponseMalformed);
    }
    let mut calendars = BTreeMap::new();
    for (calendar, observed) in response.calendars {
        if !observed.errors.is_empty()
            || observed.busy.len() > GOOGLE_CALENDAR_MAXIMUM_EVENTS
            || observed.busy.iter().any(|interval| {
                interval.start.is_empty()
                    || interval.end.is_empty()
                    || interval.start >= interval.end
                    || interval.start.len() > 128
                    || interval.end.len() > 128
            })
        {
            return Err(GoogleCalendarRefusal::ProviderResponseMalformed);
        }
        calendars.insert(calendar, observed.busy);
    }
    Ok(GoogleFreeBusyPage { calendars })
}

pub(super) fn decode_write_receipt(
    body: &[u8],
    requested: &GoogleWireEvent,
) -> Result<GoogleWriteReceipt, GoogleCalendarRefusal> {
    bounded(body)?;
    let event: GoogleWireEvent = serde_json::from_slice(body)
        .map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    validate_event(&event, true)?;
    let event_id = event
        .id
        .clone()
        .ok_or(GoogleCalendarRefusal::ProviderResponseMalformed)?;
    let revision = event
        .etag
        .clone()
        .ok_or(GoogleCalendarRefusal::ProviderResponseMalformed)?;
    if event_id.len() > GOOGLE_CALENDAR_MAXIMUM_ID_BYTES
        || revision.len() > GOOGLE_CALENDAR_MAXIMUM_ID_BYTES
    {
        return Err(GoogleCalendarRefusal::ProviderResponseMalformed);
    }
    if event.start != requested.start || event.end != requested.end {
        return Err(GoogleCalendarRefusal::TimezoneMismatch);
    }
    if event.recurrence != requested.recurrence {
        return Err(GoogleCalendarRefusal::RecurrenceMismatch);
    }
    Ok(GoogleWriteReceipt {
        event_id,
        revision,
        event,
    })
}

fn bounded(body: &[u8]) -> Result<(), GoogleCalendarRefusal> {
    if body.len() > GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES {
        Err(GoogleCalendarRefusal::ProviderResponseTooLarge)
    } else {
        Ok(())
    }
}
