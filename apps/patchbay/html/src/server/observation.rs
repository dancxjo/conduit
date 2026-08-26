//! Bounded delivery of the renderer-neutral navigation observation.

use super::{write_response, PatchbayHtmlServer, ServerError};
use std::net::TcpStream;

const MAX_NAVIGATION_OBSERVATION_BYTES: usize = 512 * 1024;

impl PatchbayHtmlServer {
    pub(super) fn write_navigation_observation(
        &self,
        stream: &mut TcpStream,
    ) -> Result<(), ServerError> {
        let observation = self
            .snapshot
            .navigation_observation()?
            .ok_or_else(|| ServerError::Interaction("portable navigation is absent".into()))?;
        let body = serde_json::to_vec(&observation)
            .map_err(|error| ServerError::Interaction(error.to_string()))?;
        if body.len() > MAX_NAVIGATION_OBSERVATION_BYTES {
            return Err(ServerError::NavigationObservationTooLarge);
        }
        write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }
}
