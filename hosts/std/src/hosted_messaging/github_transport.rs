//! HTTPS-only GitHub transport with fixed origin and finite response capture.

use std::time::Duration;

use super::github_protocol::GITHUB_MAXIMUM_RESPONSE_BYTES;
use super::{
    GitHubBearerToken, GitHubIssueCommentResponse, GitHubIssueCommentTarget,
    GitHubMessagingRefusal, GitHubMessagingTransport,
};

const GITHUB_API_ORIGIN: &str = "https://api.github.com";
const TRANSPORT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub struct GitHubHttpsTransport {
    agent: ureq::Agent,
}

impl Default for GitHubHttpsTransport {
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

impl GitHubMessagingTransport for GitHubHttpsTransport {
    fn post_comment(
        &mut self,
        credential: &GitHubBearerToken,
        target: &GitHubIssueCommentTarget,
        body: &str,
    ) -> Result<GitHubIssueCommentResponse, GitHubMessagingRefusal> {
        target.validate()?;
        let uri = format!(
            "{GITHUB_API_ORIGIN}/repos/{}/{}/issues/{}/comments",
            target.owner, target.repository, target.issue_number
        );
        let authorization = credential.authorization_value();
        let outgoing = serde_json::to_vec(&serde_json::json!({"body": body}))
            .map_err(|_| GitHubMessagingRefusal::ProviderRefused)?;
        let mut response = self
            .agent
            .post(&uri)
            .header("Authorization", &authorization)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "conduit-messaging-live-proof")
            .header("Content-Type", "application/json")
            .send(&outgoing)
            .map_err(|_| GitHubMessagingRefusal::ProviderLost)?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(GITHUB_MAXIMUM_RESPONSE_BYTES as u64 + 1)
            .read_to_vec()
            .map_err(|error| match error {
                ureq::Error::BodyExceedsLimit(_) => {
                    GitHubMessagingRefusal::ProviderResponseTooLarge
                }
                _ => GitHubMessagingRefusal::ProviderLost,
            })?;
        if body.len() > GITHUB_MAXIMUM_RESPONSE_BYTES {
            return Err(GitHubMessagingRefusal::ProviderResponseTooLarge);
        }
        Ok(GitHubIssueCommentResponse { status, body })
    }
}
