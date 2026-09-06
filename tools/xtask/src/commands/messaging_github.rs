//! Live GitHub issue-comment proof for the portable messaging seam.

use crate::{
    cli::{GlobalOpts, ProveArgs},
    process::StepError,
};
use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_messaging::{
    github_messaging_authority_grant, github_messaging_offer, github_messaging_resource_offer,
    GitHubBearerToken, GitHubHttpsTransport, GitHubIssueCommentAdapter, GitHubIssueCommentTarget,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, path::Path};

const PROOF_ID: &str = "prove.messaging-github";
const MAXIMUM_CONFIG_BYTES: usize = 4_096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessagingConfig {
    owner: String,
    repository: String,
    issue_number: u64,
    portable_recipient: String,
}

#[derive(Serialize)]
struct LiveProof {
    schema_version: u16,
    proof_class: &'static str,
    git_head: String,
    provider_profile: &'static str,
    provider_comment_id: u64,
    provider_comment_url: String,
    portable_request_sha256: String,
    request_identity: String,
    correlation_identity: String,
    authority_identity: String,
    recipient_address: String,
    recipient_address_profile: String,
    plan_id: String,
    delivery_state: String,
    evidence_kind: String,
    evidence_identity: String,
    exact_authority_grant: bool,
    exact_resource_binding: bool,
    provider_acknowledgement_only: bool,
    recipient_delivery_claimed: bool,
    provider_json_is_portable_meaning: bool,
    credential_redacted: bool,
    success: bool,
}

pub fn run(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    if opts.json || opts.quiet {
        return Err(StepError::prereq(
            PROOF_ID,
            "--json and --quiet are not supported by the interactive live messaging proof",
        ));
    }
    if opts.dry_run {
        println!(
            "messaging-github: would plan one authorized portable delivery and post one bounded issue comment"
        );
        return Ok(());
    }
    let credential = named_environment(args.credential_env.as_deref(), "--credential-env")?;
    let config_json = named_environment(
        args.messaging_config_env.as_deref(),
        "--messaging-config-env",
    )?;
    if config_json.len() > MAXIMUM_CONFIG_BYTES {
        return Err(StepError::prereq(
            PROOF_ID,
            "--messaging-config-env value exceeds its finite bound",
        ));
    }
    let config: MessagingConfig = serde_json::from_str(&config_json).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            "--messaging-config-env is not exact messaging proof JSON",
        )
    })?;
    let git_head = git_head(root)?;
    let body = format!(
        "Conduit #{} live hosted messaging proof at `{git_head}`. This GitHub response proves provider acknowledgement only; end-recipient delivery is not claimed.",
        config.issue_number
    );
    let fixture = conduit_chat::text_messaging_fixture(conduit_chat::TextMessagingFixtureSpec {
        message_identity: "message/github-live/1",
        request_identity: "delivery/github-live/1",
        correlation_identity: "correlation/github-live/1",
        authority_identity: "authority/github-live/1",
        recipient_address: &config.portable_recipient,
        recipient_address_profile: "messaging/conduit-issue@1",
        body: &body,
    })
    .map_err(|error| StepError::prereq(PROOF_ID, format!("construct request: {error:?}")))?;
    let view = conduit_chat::messaging_delivery_request_view(&fixture.request)
        .map_err(|error| StepError::prereq(PROOF_ID, format!("inspect request: {error:?}")))?;
    let plan = plan(&config)?;
    let delivery = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND)
        .ok_or_else(|| StepError::prereq(PROOF_ID, "Plan has no messaging delivery placement"))?;
    if delivery.authority.len() != 1 || delivery.resources.len() != 1 {
        return Err(StepError::prereq(
            PROOF_ID,
            "live messaging placement lacks exact authority or resource binding",
        ));
    }
    let mut adapter = GitHubIssueCommentAdapter::new(
        GitHubHttpsTransport::default(),
        GitHubBearerToken::new(credential)
            .map_err(|error| StepError::prereq(PROOF_ID, format!("credential: {error:?}")))?,
        GitHubIssueCommentTarget {
            owner: config.owner,
            repository: config.repository,
            issue_number: config.issue_number,
            portable_recipient: config.portable_recipient,
        },
    )
    .map_err(|error| StepError::prereq(PROOF_ID, format!("target: {error:?}")))?;
    let receipt = adapter
        .deliver(&fixture.request)
        .map_err(|error| StepError::prereq(PROOF_ID, format!("deliver: {error:?}")))?;
    let state = conduit_chat::messaging_delivery_state_view(&receipt.delivery.update)
        .map_err(|error| StepError::prereq(PROOF_ID, format!("inspect result: {error:?}")))?;
    if state.state != "sent" || state.evidence_kind.as_deref() != Some("provider_acknowledgement") {
        return Err(StepError::prereq(
            PROOF_ID,
            "provider response was promoted beyond supported acknowledgement evidence",
        ));
    }
    let request_bytes = fixture
        .request
        .canonical_bytes()
        .map_err(|error| StepError::prereq(PROOF_ID, format!("encode request: {error:?}")))?;
    let proof = LiveProof {
        schema_version: 1,
        proof_class: "live-transport",
        git_head,
        provider_profile: "std/messaging-github-issue-comment@1",
        provider_comment_id: receipt.provider_comment_id,
        provider_comment_url: receipt.provider_html_url,
        portable_request_sha256: format!("{:x}", Sha256::digest(request_bytes)),
        request_identity: view.request_identity,
        correlation_identity: view.correlation_identity,
        authority_identity: view
            .authority_identity
            .ok_or_else(|| StepError::prereq(PROOF_ID, "portable authority is absent"))?,
        recipient_address: view.recipients[0].address.clone(),
        recipient_address_profile: view.recipients[0].address_profile.clone(),
        plan_id: plan.plan_id.as_str().to_string(),
        delivery_state: state.state,
        evidence_kind: state.evidence_kind.unwrap_or_default(),
        evidence_identity: state.evidence_identity.unwrap_or_default(),
        exact_authority_grant: true,
        exact_resource_binding: true,
        provider_acknowledgement_only: true,
        recipient_delivery_claimed: false,
        provider_json_is_portable_meaning: false,
        credential_redacted: true,
        success: true,
    };
    let path = root.join("target/messaging-github-live.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?,
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    println!(
        "messaging-github: provider acknowledged comment {} without recipient-delivery claim ({})",
        proof.provider_comment_id,
        path.display()
    );
    Ok(())
}

fn plan(config: &MessagingConfig) -> Result<conduit_core::Plan, StepError> {
    let source = include_str!("../../../../forms/messaging-delivery/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profiles)
        .map_err(|error| StepError::prereq(PROOF_ID, error))?;
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|error| StepError::prereq(PROOF_ID, error.message))?;
    let authored = expand_canonical_form_for_authoring(&checked, "messaging-delivery", &profiles)
        .map_err(|error| StepError::prereq(PROOF_ID, error.message))?;
    let host = host(config);
    let delivery_offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND)
        .ok_or_else(|| StepError::prereq(PROOF_ID, "GitHub messaging offer is absent"))?;
    let grant = github_messaging_authority_grant(
        delivery_offer,
        "grant/github-messaging-live",
        host.host_id.clone(),
        host.boot_id.clone(),
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error))?;
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))
}

fn host(config: &MessagingConfig) -> HostAdvertisement {
    let message = conduit_std_host::hosted_messaging::messaging_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_chat::MESSAGING_MESSAGE_KIND)
        .expect("reviewed portable message offer");
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("github/{}/{}", config.owner, config.repository)),
        boot_id: BootId::from("github/messaging-live/current"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/messaging-github-live@1"),
        resources: vec![github_messaging_resource_offer()],
        planner_capabilities: vec![],
        capabilities: vec![message, github_messaging_offer()],
    }
}

fn named_environment(name: Option<&str>, flag: &str) -> Result<String, StepError> {
    let name = name
        .filter(|name| !name.is_empty())
        .ok_or_else(|| StepError::prereq(PROOF_ID, format!("{flag} is required")))?;
    env::var(name).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            format!("{flag} variable is absent or non-Unicode"),
        )
    })
}

fn git_head(root: &Path) -> Result<String, StepError> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    if !output.status.success() {
        return Err(StepError::prereq(PROOF_ID, "cannot resolve exact Git head"));
    }
    String::from_utf8(output.stdout)
        .map(|head| head.trim().to_string())
        .map_err(|_| StepError::prereq(PROOF_ID, "Git head is not UTF-8"))
}
