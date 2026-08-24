use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_std_catalog::{
    deterministic_delivery_request, deterministic_messaging_fixture, text_messaging_fixture,
    TextMessagingFixtureSpec, MESSAGING_DELIVERY_KIND,
};

use super::*;

struct FakeTransport {
    status: u16,
    body: Vec<u8>,
    observed_body: Option<String>,
}

impl GitHubMessagingTransport for FakeTransport {
    fn post_comment(
        &mut self,
        _: &GitHubBearerToken,
        _: &GitHubIssueCommentTarget,
        body: &str,
    ) -> Result<GitHubIssueCommentResponse, GitHubMessagingRefusal> {
        self.observed_body = Some(body.to_string());
        Ok(GitHubIssueCommentResponse {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

#[test]
fn exact_portable_request_maps_only_to_provider_acknowledged_sent_state() {
    let fixture = live_fixture("issue/1404");
    let transport = FakeTransport {
        status: 201,
        body: br#"{"id":5394047513,"html_url":"https://github.com/dancxjo/conduit/issues/1404#issuecomment-5394047513","extra_provider_fact":true}"#.to_vec(),
        observed_body: None,
    };
    let mut adapter = GitHubIssueCommentAdapter::new(
        transport,
        GitHubBearerToken::new("credential".into()).unwrap(),
        target(),
    )
    .unwrap();
    let receipt = adapter.deliver(&fixture.request).unwrap();
    assert_eq!(receipt.provider_comment_id, 5_394_047_513);
    assert!(receipt.provider_html_url.ends_with("5394047513"));
    assert_eq!(state_tag(&receipt.delivery.update), "sent");
    assert_eq!(
        nested_variant_tag(&receipt.delivery.update, "state"),
        "provider_acknowledgement"
    );
    assert_ne!(state_tag(&receipt.delivery.update), "delivered");
}

#[test]
fn authority_recipient_attachment_retry_and_provider_failures_stay_distinct() {
    let fixture = live_fixture("issue/1404");
    let no_authority = deterministic_delivery_request(
        &fixture.message,
        1,
        None,
        "correlation/live",
        "delivery/live",
    )
    .unwrap();
    assert_eq!(
        deliver(no_authority).unwrap_err(),
        GitHubMessagingRefusal::MissingAuthority
    );
    assert_eq!(
        deliver(live_fixture("issue/wrong").request).unwrap_err(),
        GitHubMessagingRefusal::UnsupportedRecipient
    );
    assert_eq!(
        deliver(deterministic_messaging_fixture().unwrap().request).unwrap_err(),
        GitHubMessagingRefusal::UnsupportedAttachment
    );
    let retry = deterministic_delivery_request(
        &fixture.message,
        conduit_std_catalog::MAXIMUM_DELIVERY_ATTEMPTS + 1,
        Some("authority/live"),
        "correlation/live",
        "delivery/live",
    )
    .unwrap();
    assert_eq!(
        deliver(retry).unwrap_err(),
        GitHubMessagingRefusal::RetryLimitReached
    );

    let mut adapter = adapter(FakeTransport {
        status: 403,
        body: b"{}".to_vec(),
        observed_body: None,
    });
    assert_eq!(
        adapter.deliver(&fixture.request).unwrap_err(),
        GitHubMessagingRefusal::AuthorityDenied
    );
}

#[test]
fn github_offer_preserves_portable_face_with_exact_resource_and_authority() {
    let github = github_messaging_offer();
    let deterministic = conduit_std_catalog::messaging_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == MESSAGING_DELIVERY_KIND)
        .unwrap();
    assert_eq!(github.kind_id, deterministic.kind_id);
    assert_eq!(github.inputs, deterministic.inputs);
    assert_eq!(github.outputs, deterministic.outputs);
    assert_ne!(github.implementation, deterministic.implementation);
    assert_eq!(github.authority_requirements.len(), 1);
    assert_eq!(
        github.authority_requirements[0].contract_id.as_str(),
        GITHUB_MESSAGING_AUTHORITY
    );
    assert_eq!(github.resource_requirements.len(), 1);
    assert_eq!(
        github.resource_requirements[0].class_id.as_str(),
        GITHUB_MESSAGING_RESOURCE_CLASS
    );
}

fn deliver(
    request: StructuredInfoValue,
) -> Result<GitHubIssueCommentReceipt, GitHubMessagingRefusal> {
    adapter(FakeTransport {
        status: 201,
        body: br#"{"id":1,"html_url":"https://github.com/dancxjo/conduit/issues/1404#issuecomment-1"}"#.to_vec(),
        observed_body: None,
    })
    .deliver(&request)
}

fn adapter(transport: FakeTransport) -> GitHubIssueCommentAdapter<FakeTransport> {
    GitHubIssueCommentAdapter::new(
        transport,
        GitHubBearerToken::new("credential".into()).unwrap(),
        target(),
    )
    .unwrap()
}

fn target() -> GitHubIssueCommentTarget {
    GitHubIssueCommentTarget {
        owner: "dancxjo".into(),
        repository: "conduit".into(),
        issue_number: 1404,
        portable_recipient: "issue/1404".into(),
    }
}

fn live_fixture(recipient: &str) -> conduit_std_catalog::MessagingFixture {
    text_messaging_fixture(TextMessagingFixtureSpec {
        message_identity: "message/live",
        request_identity: "delivery/live",
        correlation_identity: "correlation/live",
        authority_identity: "authority/live",
        recipient_address: recipient,
        recipient_address_profile: "messaging/conduit-issue@1",
        body: "Live provider proof.",
    })
    .unwrap()
}

fn state_tag(update: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Record(fields) = update.shape() else {
        panic!("delivery update must be a record")
    };
    let state = fields.iter().find(|field| field.name() == "state").unwrap();
    let StructuredInfoValueShape::Variant { tag, .. } = state.value().shape() else {
        panic!("delivery state must be a variant")
    };
    tag
}

fn nested_variant_tag<'a>(update: &'a StructuredInfoValue, field: &str) -> &'a str {
    let StructuredInfoValueShape::Record(fields) = update.shape() else {
        panic!("delivery update must be a record")
    };
    let value = fields.iter().find(|value| value.name() == field).unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = value.value().shape() else {
        panic!("delivery state must be a variant")
    };
    let StructuredInfoValueShape::Variant { tag, .. } = payload.shape() else {
        panic!("delivery evidence must be a variant")
    };
    tag
}
