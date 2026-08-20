use super::*;

#[derive(Default)]
struct RecordingTransport {
    requests: Vec<GoogleCalendarExchange>,
    statuses: Vec<u16>,
}

impl GoogleCalendarTransport for RecordingTransport {
    fn exchange(
        &mut self,
        credential: &GoogleBearerToken,
        request: &GoogleCalendarExchange,
    ) -> Result<GoogleCalendarResponse, GoogleCalendarRefusal> {
        assert_eq!(credential.authorization_value(), "Bearer secret-token");
        self.requests.push(request.clone());
        let status = self.statuses.pop().unwrap_or(200);
        let body = if status < 300
            && matches!(
                request.method,
                GoogleCalendarMethod::Post | GoogleCalendarMethod::Put
            )
            && request.path_and_query.contains("/events")
        {
            let mut event: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            event["id"] = serde_json::Value::String("event-7".into());
            event["etag"] = serde_json::Value::String("etag-7".into());
            serde_json::to_vec(&event).unwrap()
        } else {
            b"{}".to_vec()
        };
        Ok(GoogleCalendarResponse { status, body })
    }
}

fn client(statuses: Vec<u16>) -> GoogleCalendarClient<RecordingTransport> {
    GoogleCalendarClient::new(
        RecordingTransport {
            requests: Vec::new(),
            statuses,
        },
        GoogleBearerToken::new(b"secret-token".to_vec()).unwrap(),
        GoogleCalendarResource {
            account_identity: "account/alice".into(),
            calendar_id: "alice@example.test".into(),
        },
    )
    .unwrap()
}

fn timed_event() -> GoogleWireEvent {
    GoogleWireEvent {
        id: None,
        etag: None,
        summary: "Review".into(),
        description: String::new(),
        location: String::new(),
        start: GoogleWireEventTime {
            date: None,
            date_time: Some("2026-08-20T09:00:00-07:00".into()),
            time_zone: Some("America/Los_Angeles".into()),
        },
        end: GoogleWireEventTime {
            date: None,
            date_time: Some("2026-08-20T09:30:00-07:00".into()),
            time_zone: Some("America/Los_Angeles".into()),
        },
        attendees: Vec::new(),
        recurrence: Vec::new(),
        status: None,
    }
}

fn list_request() -> GoogleListEventsRequest {
    GoogleListEventsRequest {
        time_min: "2026-08-20T00:00:00Z".into(),
        time_max: "2026-08-21T00:00:00Z".into(),
        page_token: None,
        maximum_results: 8,
    }
}

#[test]
fn credentials_are_redacted_and_never_enter_request_values() {
    let credential = GoogleBearerToken::new(b"secret-token".to_vec()).unwrap();
    assert_eq!(format!("{credential:?}"), "GoogleBearerToken([REDACTED])");
    let mut client = client(Vec::new());
    client.list_events(&list_request()).unwrap();
    let transport = client.into_transport();
    assert_eq!(transport.requests.len(), 1);
    assert!(!format!("{:?}", transport.requests[0]).contains("secret-token"));
}

#[test]
fn read_free_busy_create_update_and_cancel_are_exact_distinct_exchanges() {
    let mut client = client(Vec::new());
    client.list_events(&list_request()).unwrap();
    client
        .query_free_busy(&GoogleFreeBusyRequest {
            time_min: "2026-08-20T00:00:00Z".into(),
            time_max: "2026-08-21T00:00:00Z".into(),
            calendar_ids: vec!["alice@example.test".into(), "bob@example.test".into()],
        })
        .unwrap();
    let write = GoogleEventWriteRequest {
        event: timed_event(),
        send_updates: false,
    };
    let created = client.create_event(&write).unwrap();
    assert_eq!(created.event_id, "event-7");
    assert_eq!(created.revision, "etag-7");
    let exact = GoogleExactEvent {
        event_id: "event-7".into(),
        revision: "etag-7".into(),
    };
    let updated = client.update_event(&exact, &write).unwrap();
    assert_eq!(updated.event_id, exact.event_id);
    assert_eq!(updated.revision, exact.revision);
    client.cancel_event(&exact, true).unwrap();
    let transport = client.into_transport();
    assert_eq!(
        transport
            .requests
            .iter()
            .map(|request| request.method)
            .collect::<Vec<_>>(),
        [
            GoogleCalendarMethod::Get,
            GoogleCalendarMethod::Post,
            GoogleCalendarMethod::Post,
            GoogleCalendarMethod::Put,
            GoogleCalendarMethod::Delete,
        ]
    );
    assert!(transport.requests[1].path_and_query.ends_with("/freeBusy"));
    assert_eq!(transport.requests[3].if_match.as_deref(), Some("etag-7"));
    assert_eq!(transport.requests[4].if_match.as_deref(), Some("etag-7"));
}

#[test]
fn provider_failures_remain_distinct_and_requests_are_bounded() {
    for (status, expected) in [
        (403, GoogleCalendarRefusal::AuthorityDenied),
        (404, GoogleCalendarRefusal::CalendarDeleted),
        (412, GoogleCalendarRefusal::StaleRevision),
        (429, GoogleCalendarRefusal::RateLimited),
        (503, GoogleCalendarRefusal::ProviderLost),
    ] {
        let mut client = client(vec![status]);
        assert_eq!(client.list_events(&list_request()), Err(expected));
    }
    let mut client = client(Vec::new());
    let mut request = list_request();
    request.maximum_results = (GOOGLE_CALENDAR_MAXIMUM_EVENTS + 1) as u16;
    assert_eq!(
        client.list_events(&request),
        Err(GoogleCalendarRefusal::Capacity)
    );
}

#[test]
fn invitation_authority_failure_is_not_collapsed_into_general_write_denial() {
    let mut client = client(vec![403]);
    assert_eq!(
        client.create_event(&GoogleEventWriteRequest {
            event: timed_event(),
            send_updates: true,
        }),
        Err(GoogleCalendarRefusal::AttendeeWriteDenied)
    );
}

#[test]
fn all_day_and_recurrence_values_survive_the_provider_request_boundary() {
    let mut event = timed_event();
    event.start = GoogleWireEventTime {
        date: Some("2026-08-20".into()),
        date_time: None,
        time_zone: None,
    };
    event.end = GoogleWireEventTime {
        date: Some("2026-08-22".into()),
        date_time: None,
        time_zone: None,
    };
    event.recurrence = vec!["RRULE:FREQ=WEEKLY;COUNT=3".into()];
    let mut client = client(Vec::new());
    client
        .create_event(&GoogleEventWriteRequest {
            event: event.clone(),
            send_updates: false,
        })
        .unwrap();
    let transport = client.into_transport();
    let encoded: GoogleWireEvent = serde_json::from_slice(&transport.requests[0].body).unwrap();
    assert_eq!(encoded.start, event.start);
    assert_eq!(encoded.end, event.end);
    assert_eq!(encoded.recurrence, event.recurrence);
}

#[test]
fn provider_normalization_cannot_silently_rewrite_time_or_recurrence() {
    let requested = timed_event();
    let mut changed_time = requested.clone();
    changed_time.id = Some("event-7".into());
    changed_time.etag = Some("etag-7".into());
    changed_time.end.date_time = Some("2026-08-20T10:00:00-07:00".into());
    assert_eq!(
        super::google_response::decode_write_receipt(
            &serde_json::to_vec(&changed_time).unwrap(),
            &requested,
        ),
        Err(GoogleCalendarRefusal::TimezoneMismatch)
    );

    let mut changed_recurrence = requested.clone();
    changed_recurrence.id = Some("event-7".into());
    changed_recurrence.etag = Some("etag-7".into());
    changed_recurrence.recurrence = vec!["RRULE:FREQ=DAILY".into()];
    assert_eq!(
        super::google_response::decode_write_receipt(
            &serde_json::to_vec(&changed_recurrence).unwrap(),
            &requested,
        ),
        Err(GoogleCalendarRefusal::RecurrenceMismatch)
    );
}
