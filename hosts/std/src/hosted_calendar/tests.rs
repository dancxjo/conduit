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
        Ok(GoogleCalendarResponse {
            status: self.statuses.pop().unwrap_or(200),
            body: b"{}".to_vec(),
        })
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
    client.create_event(&write).unwrap();
    let exact = GoogleExactEvent {
        event_id: "event-7".into(),
        revision: "etag-7".into(),
    };
    client.update_event(&exact, &write).unwrap();
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
