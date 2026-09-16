//! HTTPS-only finite retrieval for reviewed release catalogs and artifacts.

use std::time::Duration;

const MAXIMUM_RELEASE_URL_BYTES: usize = 2_048;
const RELEASE_FETCH_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedReleaseFetchRefusal {
    InvalidUrl,
    InsecureTransport,
    ProviderLost,
    HttpStatus(u16),
    BodyBound,
}

#[derive(Debug, Clone)]
pub struct HostedReleaseClient {
    agent: ureq::Agent,
}

impl Default for HostedReleaseClient {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(RELEASE_FETCH_TIMEOUT_SECONDS)))
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl HostedReleaseClient {
    pub fn get(&self, url: &str, maximum_bytes: u64) -> Result<Vec<u8>, HostedReleaseFetchRefusal> {
        if url.is_empty() || url.len() > MAXIMUM_RELEASE_URL_BYTES || maximum_bytes == 0 {
            return Err(HostedReleaseFetchRefusal::InvalidUrl);
        }
        if !url.starts_with("https://") {
            return Err(HostedReleaseFetchRefusal::InsecureTransport);
        }
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|_| HostedReleaseFetchRefusal::ProviderLost)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(HostedReleaseFetchRefusal::HttpStatus(status));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(maximum_bytes.saturating_add(1))
            .read_to_vec()
            .map_err(|error| match error {
                ureq::Error::BodyExceedsLimit(_) => HostedReleaseFetchRefusal::BodyBound,
                _ => HostedReleaseFetchRefusal::ProviderLost,
            })?;
        if bytes.is_empty() || bytes.len() as u64 > maximum_bytes {
            return Err(HostedReleaseFetchRefusal::BodyBound);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_transport_refuses_ambient_or_insecure_sources_before_io() {
        let client = HostedReleaseClient::default();
        assert_eq!(
            client.get("http://releases.example/catalog.json", 1024),
            Err(HostedReleaseFetchRefusal::InsecureTransport)
        );
        assert_eq!(
            client.get("", 1024),
            Err(HostedReleaseFetchRefusal::InvalidUrl)
        );
        assert_eq!(
            client.get("https://releases.example/catalog.json", 0),
            Err(HostedReleaseFetchRefusal::InvalidUrl)
        );
    }
}
