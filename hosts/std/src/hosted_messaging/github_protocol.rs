//! Bounded GitHub issue-comment mapping for one portable delivery request.

use conduit_chat::{
    messaging_delivery_request_view, provider_acknowledgement, DeliveryResult,
    MessagingInfoRefusal, MAXIMUM_DELIVERY_ATTEMPTS,
};
use conduit_core::StructuredInfoValue;

pub const GITHUB_MAXIMUM_COMMENT_BYTES: usize = 4_096;
pub const GITHUB_MAXIMUM_RESPONSE_BYTES: usize = 16_384;
pub const GITHUB_ISSUE_ADDRESS_PROFILE: &str = "messaging/conduit-issue@1";

#[derive(Clone)]
pub struct GitHubBearerToken(String);

impl core::fmt::Debug for GitHubBearerToken {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("GitHubBearerToken([REDACTED])")
    }
}

impl GitHubBearerToken {
    pub fn new(token: String) -> Result<Self, GitHubMessagingRefusal> {
        if token.is_empty()
            || token.len() > 512
            || token.chars().any(|character| character.is_control())
        {
            return Err(GitHubMessagingRefusal::InvalidCredential);
        }
        Ok(Self(token))
    }

    pub(crate) fn authorization_value(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssueCommentTarget {
    pub owner: String,
    pub repository: String,
    pub issue_number: u64,
    pub portable_recipient: String,
}

impl GitHubIssueCommentTarget {
    pub fn validate(&self) -> Result<(), GitHubMessagingRefusal> {
        if !valid_slug(&self.owner)
            || !valid_slug(&self.repository)
            || self.issue_number == 0
            || self.portable_recipient.is_empty()
            || self.portable_recipient.len() > 256
        {
            return Err(GitHubMessagingRefusal::InvalidTarget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssueCommentResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait GitHubMessagingTransport {
    fn post_comment(
        &mut self,
        credential: &GitHubBearerToken,
        target: &GitHubIssueCommentTarget,
        body: &str,
    ) -> Result<GitHubIssueCommentResponse, GitHubMessagingRefusal>;
}

pub struct GitHubIssueCommentAdapter<T> {
    transport: T,
    credential: GitHubBearerToken,
    target: GitHubIssueCommentTarget,
}

#[derive(Debug)]
pub struct GitHubIssueCommentReceipt {
    pub provider_comment_id: u64,
    pub provider_html_url: String,
    pub delivery: DeliveryResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubMessagingRefusal {
    InvalidCredential,
    InvalidTarget,
    InvalidPortableRequest,
    MissingAuthority,
    UnsupportedRecipient,
    UnsupportedAttachment,
    RetryLimitReached,
    MessageTooLarge,
    AuthorityDenied,
    TargetAbsent,
    RateLimited,
    ProviderRefused,
    ProviderLost,
    ProviderResponseTooLarge,
    MalformedProviderResponse,
}

impl From<MessagingInfoRefusal> for GitHubMessagingRefusal {
    fn from(_: MessagingInfoRefusal) -> Self {
        Self::InvalidPortableRequest
    }
}

impl<T: GitHubMessagingTransport> GitHubIssueCommentAdapter<T> {
    pub fn new(
        transport: T,
        credential: GitHubBearerToken,
        target: GitHubIssueCommentTarget,
    ) -> Result<Self, GitHubMessagingRefusal> {
        target.validate()?;
        Ok(Self {
            transport,
            credential,
            target,
        })
    }

    pub fn deliver(
        &mut self,
        request: &StructuredInfoValue,
    ) -> Result<GitHubIssueCommentReceipt, GitHubMessagingRefusal> {
        let view = messaging_delivery_request_view(request)?;
        if view.authority_identity.is_none() {
            return Err(GitHubMessagingRefusal::MissingAuthority);
        }
        if view.attempt > MAXIMUM_DELIVERY_ATTEMPTS {
            return Err(GitHubMessagingRefusal::RetryLimitReached);
        }
        if view.attachment_count != 0 {
            return Err(GitHubMessagingRefusal::UnsupportedAttachment);
        }
        if view.recipients.len() != 1
            || view.recipients[0].address != self.target.portable_recipient
            || view.recipients[0].address_profile != GITHUB_ISSUE_ADDRESS_PROFILE
        {
            return Err(GitHubMessagingRefusal::UnsupportedRecipient);
        }
        if view.body.is_empty() || view.body.len() > GITHUB_MAXIMUM_COMMENT_BYTES {
            return Err(GitHubMessagingRefusal::MessageTooLarge);
        }
        let response = self
            .transport
            .post_comment(&self.credential, &self.target, &view.body)?;
        if response.status != 201 {
            return Err(match response.status {
                401 | 403 => GitHubMessagingRefusal::AuthorityDenied,
                404 => GitHubMessagingRefusal::TargetAbsent,
                429 => GitHubMessagingRefusal::RateLimited,
                400 | 409 | 422 => GitHubMessagingRefusal::ProviderRefused,
                _ => GitHubMessagingRefusal::ProviderLost,
            });
        }
        if response.body.len() > GITHUB_MAXIMUM_RESPONSE_BYTES {
            return Err(GitHubMessagingRefusal::ProviderResponseTooLarge);
        }
        let provider: ProviderResponse = serde_json::from_slice(&response.body)
            .map_err(|_| GitHubMessagingRefusal::MalformedProviderResponse)?;
        let expected_prefix = format!(
            "https://github.com/{}/{}/issues/{}#issuecomment-",
            self.target.owner, self.target.repository, self.target.issue_number
        );
        if provider.id == 0
            || provider.html_url.len() > 1_024
            || !provider.html_url.starts_with(&expected_prefix)
        {
            return Err(GitHubMessagingRefusal::MalformedProviderResponse);
        }
        let delivery =
            provider_acknowledgement(request, &format!("github/issue-comment/{}", provider.id))?;
        Ok(GitHubIssueCommentReceipt {
            provider_comment_id: provider.id,
            provider_html_url: provider.html_url,
            delivery,
        })
    }
}

#[derive(serde::Deserialize)]
struct ProviderResponse {
    id: u64,
    html_url: String,
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
