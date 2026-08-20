//! HTTPS-only Google Calendar transport with fixed authority and finite I/O.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{
    GoogleBearerToken, GoogleCalendarExchange, GoogleCalendarMethod, GoogleCalendarRefusal,
    GoogleCalendarResponse, GoogleCalendarTransport, GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES,
};

const GOOGLE_CALENDAR_ORIGIN: &str = "https://www.googleapis.com";
const TRANSPORT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub struct GoogleHttpsTransport {
    agent: ureq::Agent,
}

impl Default for GoogleHttpsTransport {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(TRANSPORT_TIMEOUT_SECONDS)))
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl GoogleCalendarTransport for GoogleHttpsTransport {
    fn exchange(
        &mut self,
        credential: &GoogleBearerToken,
        request: &GoogleCalendarExchange,
    ) -> Result<GoogleCalendarResponse, GoogleCalendarRefusal> {
        request.validate()?;
        let uri = format!("{GOOGLE_CALENDAR_ORIGIN}{}", request.path_and_query);
        let authorization = credential.authorization_value();
        let mut response = match request.method {
            GoogleCalendarMethod::Get => self
                .agent
                .get(&uri)
                .header("Authorization", &authorization)
                .call(),
            GoogleCalendarMethod::Post => self
                .agent
                .post(&uri)
                .header("Authorization", &authorization)
                .header("Content-Type", "application/json")
                .send(&request.body),
            GoogleCalendarMethod::Put => {
                let mut outgoing = self
                    .agent
                    .put(&uri)
                    .header("Authorization", &authorization)
                    .header("Content-Type", "application/json");
                if let Some(revision) = &request.if_match {
                    outgoing = outgoing.header("If-Match", revision);
                }
                outgoing.send(&request.body)
            }
            GoogleCalendarMethod::Delete => {
                let mut outgoing = self
                    .agent
                    .delete(&uri)
                    .header("Authorization", &authorization);
                if let Some(revision) = &request.if_match {
                    outgoing = outgoing.header("If-Match", revision);
                }
                outgoing.call()
            }
        }
        .map_err(|_| GoogleCalendarRefusal::ProviderLost)?;

        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES as u64 + 1)
            .read_to_vec()
            .map_err(|error| match error {
                ureq::Error::BodyExceedsLimit(_) => GoogleCalendarRefusal::ProviderResponseTooLarge,
                _ => GoogleCalendarRefusal::ProviderLost,
            })?;
        if body.len() > GOOGLE_CALENDAR_MAXIMUM_BODY_BYTES {
            return Err(GoogleCalendarRefusal::ProviderResponseTooLarge);
        }
        let observed_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| GoogleCalendarRefusal::ProviderLost)?
            .as_secs();
        Ok(GoogleCalendarResponse {
            status,
            body,
            observed_unix_seconds,
        })
    }
}
