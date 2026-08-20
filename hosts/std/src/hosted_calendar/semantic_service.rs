//! Provider-neutral semantic requests mapped onto one selected Google resource.

use serde::{Deserialize, Serialize};

use super::{
    CalendarHostedOperation, GoogleCalendarClient, GoogleCalendarRefusal, GoogleCalendarTransport,
    GoogleEventWriteRequest, GoogleExactEvent, GoogleFreeBusyRequest, GoogleListEventsRequest,
    GoogleWireAttendee, GoogleWireBusyInterval, GoogleWireEvent, GoogleWireEventTime,
    GOOGLE_CALENDAR_MAXIMUM_CALENDARS, GOOGLE_CALENDAR_MAXIMUM_EVENTS,
    GOOGLE_CALENDAR_MAXIMUM_RECURRENCE_RULES,
};

const MAXIMUM_FREE_BUSY_AGE_SECONDS: u64 = 3_600;

pub trait HostedCalendarAdapter: Send {
    fn execute(
        &mut self,
        operation: CalendarHostedOperation,
        semantic_json: &[u8],
        prior_realization_json: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal>;
}

pub struct GoogleCalendarService<T> {
    client: GoogleCalendarClient<T>,
}

impl<T: GoogleCalendarTransport> GoogleCalendarService<T> {
    pub fn new(client: GoogleCalendarClient<T>) -> Self {
        Self { client }
    }
}

impl<T: GoogleCalendarTransport> HostedCalendarAdapter for GoogleCalendarService<T> {
    fn execute(
        &mut self,
        operation: CalendarHostedOperation,
        semantic_json: &[u8],
        prior_realization_json: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        match operation {
            CalendarHostedOperation::Read => self.read(semantic_json),
            CalendarHostedOperation::FreeBusy => self.free_busy(semantic_json),
            CalendarHostedOperation::Create => self.create(semantic_json),
            CalendarHostedOperation::Update => self.update(semantic_json, prior_realization_json),
            CalendarHostedOperation::Cancel => self.cancel(semantic_json, prior_realization_json),
            CalendarHostedOperation::Invite => self.invite(semantic_json, prior_realization_json),
        }
    }
}

impl<T: GoogleCalendarTransport> GoogleCalendarService<T> {
    fn read(&mut self, input: &[u8]) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: ReadSemantic = decode(input)?;
        let page = self.client.list_events(&GoogleListEventsRequest {
            time_min: request.time_min,
            time_max: request.time_max,
            page_token: request.page_token,
            maximum_results: request.maximum_results,
        })?;
        encode(&ReadRealization {
            resource: resource_realization(&self.client),
            events: page.events,
            next_page_token: page.next_page_token,
        })
    }

    fn free_busy(&mut self, input: &[u8]) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: FreeBusySemantic = decode(input)?;
        if request.participants.is_empty()
            || request.participants.len() > GOOGLE_CALENDAR_MAXIMUM_CALENDARS
            || request.maximum_age_seconds == 0
            || request.maximum_age_seconds > MAXIMUM_FREE_BUSY_AGE_SECONDS
            || request.participants.iter().any(|participant| {
                participant.participant_identity.is_empty()
                    || participant.contact_reference.is_empty()
            })
        {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        let calendar_ids = request
            .participants
            .iter()
            .map(|participant| participant.contact_reference.clone())
            .collect::<Vec<_>>();
        let page = self.client.query_free_busy(&GoogleFreeBusyRequest {
            time_min: request.time_min,
            time_max: request.time_max,
            calendar_ids,
        })?;
        let usable_until = page
            .observed_unix_seconds
            .checked_add(request.maximum_age_seconds)
            .ok_or(GoogleCalendarRefusal::StaleFreeBusy)?;
        if request.reference_unix_seconds > usable_until {
            return Err(GoogleCalendarRefusal::StaleFreeBusy);
        }
        let mut participants = Vec::with_capacity(request.participants.len());
        for participant in request.participants {
            let busy = page
                .calendars
                .get(&participant.contact_reference)
                .cloned()
                .ok_or(GoogleCalendarRefusal::ProviderResponseMalformed)?;
            participants.push(ParticipantBusyRealization {
                participant_identity: participant.participant_identity,
                busy,
            });
        }
        encode(&FreeBusyRealization {
            resource: resource_realization(&self.client),
            observed_unix_seconds: page.observed_unix_seconds,
            usable_until_unix_seconds: usable_until,
            participants,
        })
    }

    fn create(&mut self, input: &[u8]) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: CreateSemantic = decode(input)?;
        let event = wire_event(&request.event)?;
        let provider_receipt = self.client.create_event(&GoogleEventWriteRequest {
            event,
            send_updates: false,
        })?;
        encode(&receipt(
            resource_realization(&self.client),
            request.event.identity,
            provider_receipt,
        ))
    }

    fn update(
        &mut self,
        input: &[u8],
        prior: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: UpdateSemantic = decode(input)?;
        let prior = prior_receipt(prior)?;
        if prior.portable_event_identity != request.event.identity {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        let updated = self.client.update_event(
            &exact(&prior),
            &GoogleEventWriteRequest {
                event: wire_event(&request.event)?,
                send_updates: false,
            },
        )?;
        encode(&receipt(
            resource_realization(&self.client),
            request.event.identity,
            updated,
        ))
    }

    fn cancel(
        &mut self,
        input: &[u8],
        prior: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: CancelSemantic = decode(input)?;
        let prior = prior_receipt(prior)?;
        if prior.portable_event_identity != request.event_identity || request.notify_participants {
            return Err(GoogleCalendarRefusal::AttendeeWriteDenied);
        }
        self.client.cancel_event(&exact(&prior), false)?;
        encode(&CancelRealization {
            resource: resource_realization(&self.client),
            portable_event_identity: prior.portable_event_identity,
            provider_event_id: prior.provider_event_id,
            cancelled_revision: prior.provider_revision,
        })
    }

    fn invite(
        &mut self,
        input: &[u8],
        prior: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let request: InviteSemantic = decode(input)?;
        let prior = prior_receipt(prior)?;
        if prior.portable_event_identity != request.event_identity
            || request.participant_contacts.is_empty()
            || request.participant_contacts.len() > conduit_core::MAXIMUM_EVENT_PARTICIPANTS
        {
            return Err(GoogleCalendarRefusal::InvalidRequest);
        }
        let mut event = prior.event.clone();
        event.attendees = request
            .participant_contacts
            .into_iter()
            .map(|email| GoogleWireAttendee {
                email,
                response_status: None,
                optional: false,
                organizer: false,
            })
            .collect();
        let updated = self.client.update_event(
            &exact(&prior),
            &GoogleEventWriteRequest {
                event,
                send_updates: true,
            },
        )?;
        encode(&receipt(
            resource_realization(&self.client),
            prior.portable_event_identity,
            updated,
        ))
    }
}

#[derive(Deserialize)]
struct ReadSemantic {
    time_min: String,
    time_max: String,
    #[serde(default)]
    page_token: Option<String>,
    maximum_results: u16,
}

#[derive(Deserialize)]
struct FreeBusySemantic {
    time_min: String,
    time_max: String,
    reference_unix_seconds: u64,
    maximum_age_seconds: u64,
    participants: Vec<ParticipantReference>,
}

#[derive(Deserialize)]
struct ParticipantReference {
    participant_identity: String,
    contact_reference: String,
}

#[derive(Deserialize)]
struct CreateSemantic {
    event: EventSemantic,
}

#[derive(Deserialize)]
struct UpdateSemantic {
    event: EventSemantic,
}

#[derive(Deserialize)]
struct CancelSemantic {
    event_identity: String,
    #[serde(default)]
    notify_participants: bool,
}

#[derive(Deserialize)]
struct InviteSemantic {
    event_identity: String,
    participant_contacts: Vec<String>,
}

#[derive(Deserialize)]
struct EventSemantic {
    identity: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    location: String,
    time: EventTimeSemantic,
    #[serde(default)]
    recurrence: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EventTimeSemantic {
    Timed {
        start: String,
        end: String,
        time_zone: String,
    },
    AllDay {
        start: String,
        end_exclusive: String,
    },
}

#[derive(Serialize)]
struct ReadRealization {
    resource: ResourceRealization,
    events: Vec<GoogleWireEvent>,
    next_page_token: Option<String>,
}

#[derive(Serialize)]
struct FreeBusyRealization {
    resource: ResourceRealization,
    observed_unix_seconds: u64,
    usable_until_unix_seconds: u64,
    participants: Vec<ParticipantBusyRealization>,
}

#[derive(Serialize)]
struct ParticipantBusyRealization {
    participant_identity: String,
    busy: Vec<GoogleWireBusyInterval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WriteRealization {
    resource: ResourceRealization,
    portable_event_identity: String,
    provider_event_id: String,
    provider_revision: String,
    event: GoogleWireEvent,
}

#[derive(Serialize)]
struct CancelRealization {
    resource: ResourceRealization,
    portable_event_identity: String,
    provider_event_id: String,
    cancelled_revision: String,
}

fn wire_event(event: &EventSemantic) -> Result<GoogleWireEvent, GoogleCalendarRefusal> {
    if event.identity.is_empty()
        || event.title.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.description.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.location.len() > conduit_core::MAXIMUM_CALENDAR_TEXT_BYTES
        || event.recurrence.len() > GOOGLE_CALENDAR_MAXIMUM_RECURRENCE_RULES
    {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    }
    let (start, end) = match &event.time {
        EventTimeSemantic::Timed {
            start,
            end,
            time_zone,
        } => (
            GoogleWireEventTime {
                date: None,
                date_time: Some(start.clone()),
                time_zone: Some(time_zone.clone()),
            },
            GoogleWireEventTime {
                date: None,
                date_time: Some(end.clone()),
                time_zone: Some(time_zone.clone()),
            },
        ),
        EventTimeSemantic::AllDay {
            start,
            end_exclusive,
        } => (
            GoogleWireEventTime {
                date: Some(start.clone()),
                date_time: None,
                time_zone: None,
            },
            GoogleWireEventTime {
                date: Some(end_exclusive.clone()),
                date_time: None,
                time_zone: None,
            },
        ),
    };
    Ok(GoogleWireEvent {
        id: None,
        etag: None,
        summary: event.title.clone(),
        description: event.description.clone(),
        location: event.location.clone(),
        start,
        end,
        attendees: Vec::new(),
        recurrence: event.recurrence.clone(),
        status: None,
    })
}

fn prior_receipt(prior: Option<&[u8]>) -> Result<WriteRealization, GoogleCalendarRefusal> {
    let prior = prior.ok_or(GoogleCalendarRefusal::InvalidRequest)?;
    if prior.is_empty() || prior.len() > conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES as usize
    {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    }
    serde_json::from_slice(prior).map_err(|_| GoogleCalendarRefusal::InvalidRequest)
}

fn exact(receipt: &WriteRealization) -> GoogleExactEvent {
    GoogleExactEvent {
        event_id: receipt.provider_event_id.clone(),
        revision: receipt.provider_revision.clone(),
    }
}

fn receipt(
    resource: ResourceRealization,
    portable_event_identity: String,
    receipt: super::GoogleWriteReceipt,
) -> WriteRealization {
    WriteRealization {
        resource,
        portable_event_identity,
        provider_event_id: receipt.event_id,
        provider_revision: receipt.revision,
        event: receipt.event,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceRealization {
    account_identity: String,
    calendar_id: String,
}

fn resource_realization<T: GoogleCalendarTransport>(
    client: &GoogleCalendarClient<T>,
) -> ResourceRealization {
    ResourceRealization {
        account_identity: client.resource().account_identity.clone(),
        calendar_id: client.resource().calendar_id.clone(),
    }
}

fn decode<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T, GoogleCalendarRefusal> {
    if input.is_empty()
        || input.len() > conduit_std_catalog::CALENDAR_MAXIMUM_SEMANTIC_JSON_BYTES as usize
    {
        return Err(GoogleCalendarRefusal::InvalidRequest);
    }
    serde_json::from_slice(input).map_err(|_| GoogleCalendarRefusal::InvalidRequest)
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, GoogleCalendarRefusal> {
    let encoded =
        serde_json::to_vec(value).map_err(|_| GoogleCalendarRefusal::ProviderResponseMalformed)?;
    if encoded.len() > conduit_std_catalog::CALENDAR_MAXIMUM_RESULT_BYTES as usize
        || encoded.len() > GOOGLE_CALENDAR_MAXIMUM_EVENTS * 1_024
    {
        Err(GoogleCalendarRefusal::ProviderResponseTooLarge)
    } else {
        Ok(encoded)
    }
}
