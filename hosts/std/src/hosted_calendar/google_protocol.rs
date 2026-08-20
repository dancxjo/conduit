//! Bounded Google Calendar protocol and credential/resource boundary.
//!
//! These types are realization facts. They do not belong in authored Forms or
//! portable calendar Info, and this module owns no scheduling or retry policy.
use serde::{Deserialize, Serialize};
use std::fmt;

pub const GOOGLE_CALENDAR_MAXIMUM_TOKEN_BYTES: usize = 8_192;
pub const GOOGLE_CALENDAR_MAXIMUM_ID_BYTES: usize = 1_024;
pub const GOOGLE_CALENDAR_MAXIMUM_PAGE_TOKEN_BYTES: usize = 2_048;
pub const GOOGLE_CALENDAR_MAXIMUM_EVENTS: usize = 64;
pub const GOOGLE_CALENDAR_MAXIMUM_CALENDARS: usize = 16;
pub const GOOGLE_CALENDAR_MAXIMUM_RECURRENCE_RULES: usize = 16;
pub const GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES: usize = 256 * 1_024;
#[derive(Clone, PartialEq, Eq)]
pub struct GoogleBearerToken(Vec<u8>);

impl GoogleBearerToken {
    pub fn new(value: Vec<u8>) -> Result<Self, GoogleCalendarRefusal> {
        if value.is_empty()
            || value.len() > GOOGLE_CALENDAR_MAXIMUM_TOKEN_BYTES
            || value.iter().any(|byte| !byte.is_ascii_graphic())
        {
            return Err(GoogleCalendarRefusal::InvalidCredential);
        }
        Ok(Self(value))
    }

    pub(crate) fn authorization_value(&self) -> String {
        let token = std::str::from_utf8(&self.0).expect("validated Google token is ASCII");
        let mut value = String::with_capacity(7 + self.0.len());
        value.push_str("Bearer ");
        value.push_str(token);
        value
    }
}

impl fmt::Debug for GoogleBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GoogleBearerToken([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleCalendarResource {
    pub account_identity: String,
    pub calendar_id: String,
}

impl GoogleCalendarResource {
    pub fn validate(&self) -> Result<(), GoogleCalendarRefusal> {
        validate_id(&self.account_identity)?;
        validate_id(&self.calendar_id)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GoogleCalendarMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleCalendarExchange {
    pub method: GoogleCalendarMethod,
    pub path_and_query: String,
    pub body: Vec<u8>,
    pub if_match: Option<String>,
}

impl GoogleCalendarExchange {
    pub fn validate(&self) -> Result<(), GoogleCalendarRefusal> {
        if !self.path_and_query.starts_with("/calendar/v3/")
            || self.path_and_query.len() > 8_192
            || self.body.len() > GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES
            || self.if_match.as_ref().is_some_and(|value| {
                value.is_empty()
                    || value.len() > GOOGLE_CALENDAR_MAXIMUM_ID_BYTES
                    || value.contains(['\r', '\n'])
            })
        {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleCalendarResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub observed_unix_seconds: u64,
}

pub trait GoogleCalendarTransport: Send {
    fn exchange(
        &mut self,
        credential: &GoogleBearerToken,
        request: &GoogleCalendarExchange,
    ) -> Result<GoogleCalendarResponse, GoogleCalendarRefusal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleListEventsRequest {
    pub time_min: String,
    pub time_max: String,
    pub page_token: Option<String>,
    pub maximum_results: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleFreeBusyRequest {
    pub time_min: String,
    pub time_max: String,
    pub calendar_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleEventWriteRequest {
    pub event: GoogleWireEvent,
    pub send_updates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleExactEvent {
    pub event_id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWireEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub location: String,
    pub start: GoogleWireEventTime,
    pub end: GoogleWireEventTime,
    #[serde(default)]
    pub attendees: Vec<GoogleWireAttendee>,
    #[serde(default)]
    pub recurrence: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWireEventTime {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub date_time: Option<String>,
    #[serde(default)]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWireAttendee {
    pub email: String,
    #[serde(default)]
    pub response_status: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub organizer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWireBusyInterval {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWireCalendarBusy {
    #[serde(default)]
    pub busy: Vec<GoogleWireBusyInterval>,
    #[serde(default)]
    pub errors: Vec<GoogleWireError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleWireError {
    pub domain: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleCalendarRefusal {
    InvalidCredential,
    InvalidResource,
    InvalidRequest,
    Capacity,
    AuthorityDenied,
    StaleRevision,
    CalendarDeleted,
    EventDeleted,
    AttendeeWriteDenied,
    RateLimited,
    ProviderLost,
    ProviderResponseMalformed,
    ProviderResponseTooLarge,
    TimezoneMismatch,
    RecurrenceMismatch,
    StaleFreeBusy,
}

pub struct GoogleCalendarClient<T> {
    transport: T,
    credential: GoogleBearerToken,
    resource: GoogleCalendarResource,
}

impl<T: GoogleCalendarTransport> GoogleCalendarClient<T> {
    pub fn new(
        transport: T,
        credential: GoogleBearerToken,
        resource: GoogleCalendarResource,
    ) -> Result<Self, GoogleCalendarRefusal> {
        resource.validate()?;
        Ok(Self {
            transport,
            credential,
            resource,
        })
    }

    pub fn list_events(
        &mut self,
        request: &GoogleListEventsRequest,
    ) -> Result<super::GoogleEventPage, GoogleCalendarRefusal> {
        if request.maximum_results == 0
            || usize::from(request.maximum_results) > GOOGLE_CALENDAR_MAXIMUM_EVENTS
        {
            return Err(GoogleCalendarRefusal::Capacity);
        }
        validate_rfc3339_bound(&request.time_min)?;
        validate_rfc3339_bound(&request.time_max)?;
        if request.page_token.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > GOOGLE_CALENDAR_MAXIMUM_PAGE_TOKEN_BYTES
        }) {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        let mut path = format!(
            "/calendar/v3/calendars/{}/events?singleEvents=true&maxResults={}&timeMin={}&timeMax={}",
            percent_encode(&self.resource.calendar_id),
            request.maximum_results,
            percent_encode(&request.time_min),
            percent_encode(&request.time_max)
        );
        if let Some(page_token) = &request.page_token {
            path.push_str("&pageToken=");
            path.push_str(&percent_encode(page_token));
        }
        let response = self.perform(GoogleCalendarExchange {
            method: GoogleCalendarMethod::Get,
            path_and_query: path,
            body: Vec::new(),
            if_match: None,
        })?;
        super::google_response::decode_event_page(&response.body, request.maximum_results)
    }

    pub fn query_free_busy(
        &mut self,
        request: &GoogleFreeBusyRequest,
    ) -> Result<super::GoogleFreeBusyPage, GoogleCalendarRefusal> {
        if request.calendar_ids.is_empty()
            || request.calendar_ids.len() > GOOGLE_CALENDAR_MAXIMUM_CALENDARS
            || request
                .calendar_ids
                .iter()
                .any(|value| validate_id(value).is_err())
        {
            return Err(GoogleCalendarRefusal::Capacity);
        }
        validate_rfc3339_bound(&request.time_min)?;
        validate_rfc3339_bound(&request.time_max)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "timeMin": request.time_min,
            "timeMax": request.time_max,
            "items": request.calendar_ids.iter().map(|id| serde_json::json!({"id": id})).collect::<Vec<_>>(),
        }))
        .map_err(|_| GoogleCalendarRefusal::InvalidRequest)?;
        let response = self.perform(GoogleCalendarExchange {
            method: GoogleCalendarMethod::Post,
            path_and_query: "/calendar/v3/freeBusy".into(),
            body,
            if_match: None,
        })?;
        super::google_response::decode_free_busy(
            &response.body,
            &request.calendar_ids,
            response.observed_unix_seconds,
        )
    }

    pub fn create_event(
        &mut self,
        request: &GoogleEventWriteRequest,
    ) -> Result<super::GoogleWriteReceipt, GoogleCalendarRefusal> {
        validate_event(&request.event, false)?;
        let updates = if request.send_updates { "all" } else { "none" };
        let response = self.perform(GoogleCalendarExchange {
            method: GoogleCalendarMethod::Post,
            path_and_query: format!(
                "/calendar/v3/calendars/{}/events?sendUpdates={updates}",
                percent_encode(&self.resource.calendar_id)
            ),
            body: serde_json::to_vec(&request.event)
                .map_err(|_| GoogleCalendarRefusal::InvalidRequest)?,
            if_match: None,
        })?;
        super::google_response::decode_write_receipt(&response.body, &request.event)
    }

    pub fn update_event(
        &mut self,
        exact: &GoogleExactEvent,
        request: &GoogleEventWriteRequest,
    ) -> Result<super::GoogleWriteReceipt, GoogleCalendarRefusal> {
        validate_exact_event(exact)?;
        validate_event(&request.event, false)?;
        let updates = if request.send_updates { "all" } else { "none" };
        let response = self.perform(GoogleCalendarExchange {
            method: GoogleCalendarMethod::Put,
            path_and_query: format!(
                "/calendar/v3/calendars/{}/events/{}?sendUpdates={updates}",
                percent_encode(&self.resource.calendar_id),
                percent_encode(&exact.event_id)
            ),
            body: serde_json::to_vec(&request.event)
                .map_err(|_| GoogleCalendarRefusal::InvalidRequest)?,
            if_match: Some(exact.revision.clone()),
        })?;
        super::google_response::decode_write_receipt(&response.body, &request.event)
    }

    pub fn cancel_event(
        &mut self,
        exact: &GoogleExactEvent,
        send_updates: bool,
    ) -> Result<(), GoogleCalendarRefusal> {
        validate_exact_event(exact)?;
        let updates = if send_updates { "all" } else { "none" };
        self.perform(GoogleCalendarExchange {
            method: GoogleCalendarMethod::Delete,
            path_and_query: format!(
                "/calendar/v3/calendars/{}/events/{}?sendUpdates={updates}",
                percent_encode(&self.resource.calendar_id),
                percent_encode(&exact.event_id)
            ),
            body: Vec::new(),
            if_match: Some(exact.revision.clone()),
        })?;
        Ok(())
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn resource(&self) -> &GoogleCalendarResource {
        &self.resource
    }

    fn perform(
        &mut self,
        request: GoogleCalendarExchange,
    ) -> Result<GoogleCalendarResponse, GoogleCalendarRefusal> {
        request.validate()?;
        let response = self.transport.exchange(&self.credential, &request)?;
        if response.body.len() > GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES {
            return Err(GoogleCalendarRefusal::ProviderResponseTooLarge);
        }
        match response.status {
            200..=299 => Ok(response),
            401 => Err(GoogleCalendarRefusal::AuthorityDenied),
            403 if request.path_and_query.contains("sendUpdates=all") => {
                Err(GoogleCalendarRefusal::AttendeeWriteDenied)
            }
            403 => Err(GoogleCalendarRefusal::AuthorityDenied),
            404 if request.path_and_query.contains("/events/") => {
                Err(GoogleCalendarRefusal::EventDeleted)
            }
            404 => Err(GoogleCalendarRefusal::CalendarDeleted),
            409 | 412 => Err(GoogleCalendarRefusal::StaleRevision),
            429 => Err(GoogleCalendarRefusal::RateLimited),
            500..=599 => Err(GoogleCalendarRefusal::ProviderLost),
            _ => Err(GoogleCalendarRefusal::ProviderResponseMalformed),
        }
    }
}

pub(super) fn validate_event(
    event: &GoogleWireEvent,
    requires_provider_identity: bool,
) -> Result<(), GoogleCalendarRefusal> {
    if event.summary.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.description.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.location.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.attendees.len() > conduit_core::MAXIMUM_EVENT_PARTICIPANTS
        || event.recurrence.len() > GOOGLE_CALENDAR_MAXIMUM_RECURRENCE_RULES
        || (requires_provider_identity
            && event
                .id
                .as_ref()
                .is_none_or(|value| validate_id(value).is_err()))
        || event
            .etag
            .as_ref()
            .is_some_and(|value| validate_id(value).is_err())
    {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    }
    validate_event_time_pair(&event.start, &event.end)?;
    Ok(())
}

fn validate_event_time_pair(
    start: &GoogleWireEventTime,
    end: &GoogleWireEventTime,
) -> Result<(), GoogleCalendarRefusal> {
    match (
        start.date.as_ref(),
        start.date_time.as_ref(),
        end.date.as_ref(),
        end.date_time.as_ref(),
    ) {
        (Some(start_date), None, Some(end_date), None) if start_date < end_date => Ok(()),
        (None, Some(start_date_time), None, Some(end_date_time))
            if start_date_time < end_date_time =>
        {
            if start_date_time.is_empty()
                || end_date_time.is_empty()
                || start_date_time.len() > 128
                || end_date_time.len() > 128
                || start.time_zone != end.time_zone
            {
                Err(GoogleCalendarRefusal::TimezoneMismatch)
            } else {
                Ok(())
            }
        }
        _ => Err(GoogleCalendarRefusal::TimezoneMismatch),
    }
}

fn validate_exact_event(exact: &GoogleExactEvent) -> Result<(), GoogleCalendarRefusal> {
    validate_id(&exact.event_id)?;
    validate_id(&exact.revision)
}

fn validate_id(value: &str) -> Result<(), GoogleCalendarRefusal> {
    if value.is_empty()
        || value.len() > GOOGLE_CALENDAR_MAXIMUM_ID_BYTES
        || value.contains(['\r', '\n'])
    {
        Err(GoogleCalendarRefusal::InvalidResource)
    } else {
        Ok(())
    }
}

fn validate_rfc3339_bound(value: &str) -> Result<(), GoogleCalendarRefusal> {
    if value.len() < 20
        || value.len() > 128
        || !value.contains('T')
        || !(value.ends_with('Z') || value.rfind(['+', '-']).is_some_and(|index| index > 9))
    {
        Err(GoogleCalendarRefusal::InvalidRequest)
    } else {
        Ok(())
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}
