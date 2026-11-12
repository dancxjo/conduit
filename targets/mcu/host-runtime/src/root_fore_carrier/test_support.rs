//! Exact signed preparation fixtures, with no carrier or runtime grant.
use conduit_body::*;
use conduit_core as c;
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use std::format;
use std::prelude::v1::*;
#[allow(dead_code)]
#[path = "test_auth.rs"]
mod auth;
pub use auth::Fixture;
pub fn id(value: &str) -> c::AssignedIdentity {
    c::AssignedIdentity::from_text(value)
}
pub fn local() -> c::AssignedForeRequirement {
    c::AssignedForeRequirement::new(2, 7, c::AssignedForeDirection::Output, id("local"), [2; 32])
}
pub fn remote() -> c::AssignedForeRequirement {
    c::AssignedForeRequirement::new(
        1,
        6,
        c::AssignedForeDirection::Input,
        id("read_index"),
        [3; 32],
    )
}
pub fn binding() -> c::AssignedRemoteBinding {
    c::AssignedRemoteBinding {
        line: id("line"),
        local_host: id("host/1"),
        local_boot: id("boot/1"),
        peer_host: id("peer"),
        peer_boot: id("peer-boot"),
    }
}
pub fn line(digest: u8) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        remote(),
        binding(),
        id("session"),
        id("session-profile"),
        [digest; 32],
    )
}
pub fn profile<'a>(
    local: &'a [c::AssignedForeRequirement],
    lines: &'a [c::AssignedLineForeRequirement],
    bindings: &'a [c::AssignedRemoteBinding],
) -> AssignedBodyLineForePreparationProfile<'a> {
    let mut maxima = c::AssignedPlanMaxima::TINY_HOST;
    maxima.encoded_bytes = 4096;
    maxima.counts[9] = 2;
    AssignedBodyLineForePreparationProfile {
        preparation: AssignedBodyPreparationProfile {
            maxima,
            requirements: c::AssignedPlanRequirements {
                host: id("host/1"),
                boot: id("boot/1"),
                host_calls: &[],
                resources: &[],
                remote_bindings: bindings,
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: 0,
                maximum_total_text_bytes: 0,
                requirements: &[],
            },
            fore: None,
        },
        line_fore: c::AssignedLineForeProfile { local, lines },
    }
}
pub fn projection(schema: u16) -> Vec<u8> {
    let mut bytes = vec![0; c::ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&c::assigned_plan_magic());
    bytes[8..10].copy_from_slice(&schema.to_le_bytes());
    bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
    for (start, value) in [
        (16, "plot-plan"),
        (32, "fragment"),
        (48, "host/1"),
        (64, "boot/1"),
    ] {
        bytes[start..start + 16].copy_from_slice(&id(value).0);
    }
    bytes[89] = 2;
    // Record order deliberately differs from the metadata's endpoint order.
    let payloads = if schema == c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA {
        [local().payload().to_vec(), remote().payload().to_vec()]
    } else {
        [
            c::assigned_line_fore_local_payload(&local()).to_vec(),
            line(7).payload().to_vec(),
        ]
    };
    for payload in payloads {
        bytes.push(c::ASSIGNED_REMOTE_ENDPOINT);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&payload);
    }
    let n = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&n.to_le_bytes());
    let digest = c::assigned_plan_payload_digest(&bytes[125..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
pub fn document(
    projection: &[u8],
    body: &str,
    boot: &str,
    fore: &[u8],
    resource: &[u8],
) -> Vec<u8> {
    document_for(
        projection,
        body,
        boot,
        fore,
        resource,
        "fragment",
        &["fragment"],
    )
}
pub fn document_for(
    projection: &[u8],
    body: &str,
    boot: &str,
    fore: &[u8],
    resource: &[u8],
    fragment: &str,
    local_ids: &[&str],
) -> Vec<u8> {
    let play = format!(
        "body-play/sha256:{}",
        body_play_digest(body, "wake", "body-plan", 1)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let (local_count, local_digest) = local_fragment_set_commitment(local_ids).unwrap();
    let metadata = BodyFragmentApprovalMetadata {
        schema: 3,
        body,
        wake: "wake",
        body_plan: "body-plan",
        body_play: &play,
        play_sequence: 1,
        workload_revision: 1,
        source: "source",
        checked_plot: "checked",
        expanded_plot: "expanded",
        plot_plan: "plot-plan",
        fragment,
        host: "host/1",
        boot,
        offer_generation: 1,
        local_fragment_count: local_count,
        local_fragment_digest: local_digest,
        resource_bindings: resource,
        fore_ports: fore,
        projection_length: projection.len() as u32,
        projection_digest: Sha256::digest(projection).into(),
    };
    encode_body_fragment_document(&metadata, projection, 4096).unwrap()
}
pub fn inventory() -> Vec<u8> {
    [remote().payload().as_slice(), local().payload().as_slice()].concat()
}
pub fn slot(f: &Fixture) -> IssuedHostCommandChallenge<'_> {
    IssuedHostCommandChallenge::issue(
        &f.member,
        &f.host,
        &f.issuer,
        HostCommandReceiveBounds {
            payload_contract: HOST_PREPARED_FRAGMENT_CONTRACT,
            maximum_payload_bytes: 4096,
            lifetime_millis: 10,
        },
        [1; 32],
        10,
    )
    .unwrap()
}
pub fn command<'a>(f: &'a Fixture, payload: &'a [u8]) -> AttributedHostCommand<'a, 'a> {
    let slot = slot(f);
    let signature = f
        .key
        .sign(&host_command_attribution_transcript(slot.challenge(), payload).unwrap())
        .to_bytes();
    slot.authenticate(payload, &signature, 11).unwrap()
}

pub fn workset_command<'a>(f: &'a Fixture, payload: &'a [u8]) -> AttributedHostCommand<'a, 'a> {
    let slot = IssuedHostCommandChallenge::issue(
        &f.member,
        &f.host,
        &f.issuer,
        HostCommandReceiveBounds {
            payload_contract: HOST_PREPARED_WORKSET_CONTRACT,
            maximum_payload_bytes: 4096,
            lifetime_millis: 10,
        },
        [1; 32],
        10,
    )
    .unwrap();
    let signature = f
        .key
        .sign(&host_command_attribution_transcript(slot.challenge(), payload).unwrap())
        .to_bytes();
    slot.authenticate(payload, &signature, 11).unwrap()
}
