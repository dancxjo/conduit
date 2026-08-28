//! Hosted messaging mechanisms below the portable message/delivery seam.

mod github_protocol;
mod github_transport;
mod offer;

#[cfg(test)]
mod tests;

pub use github_protocol::{
    GitHubBearerToken, GitHubIssueCommentAdapter, GitHubIssueCommentReceipt,
    GitHubIssueCommentResponse, GitHubIssueCommentTarget, GitHubMessagingRefusal,
    GitHubMessagingTransport,
};
pub use github_transport::GitHubHttpsTransport;
pub use offer::{
    github_messaging_authority_grant, github_messaging_offer, github_messaging_resource_offer,
    messaging_std_offers, GITHUB_MESSAGING_AUTHORITY, GITHUB_MESSAGING_RESOURCE_CLASS,
    MESSAGING_DELIVERY_AUTHORITY, MESSAGING_HOST_OPERATION,
};
