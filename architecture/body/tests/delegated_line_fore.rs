#![cfg(feature = "authenticated-admission")]
//! Authenticated preparation only: these fixtures own no carrier or runtime grant.
use conduit_body::*;
use conduit_core as c;
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
#[allow(dead_code)] // Shared auth fixture also supplies a small-command helper.
#[path = "host_preparation_delegation/support.rs"]
mod support;
use support::Fixture;

fn id(value: &str) -> c::AssignedIdentity {
    c::AssignedIdentity::from_text(value)
}
fn local() -> c::AssignedForeRequirement {
    c::AssignedForeRequirement::new(2, 7, c::AssignedForeDirection::Output, id("local"), [2; 32])
}
fn remote() -> c::AssignedForeRequirement {
    c::AssignedForeRequirement::new(
        1,
        6,
        c::AssignedForeDirection::Input,
        id("read_index"),
        [3; 32],
    )
}
fn binding() -> c::AssignedRemoteBinding {
    c::AssignedRemoteBinding {
        line: id("line"),
        local_host: id("host/1"),
        local_boot: id("boot/1"),
        peer_host: id("peer"),
        peer_boot: id("peer-boot"),
    }
}
fn line(digest: u8) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        remote(),
        binding(),
        id("session"),
        id("session-profile"),
        [digest; 32],
    )
}
fn policy() -> HostPreparationDelegation {
    HostPreparationDelegation {
        maximum_assignment_bytes: 4096,
        ..support::policy()
    }
}
fn profile<'a>(
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
fn projection(schema: u16) -> Vec<u8> {
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
    for payload in [
        c::assigned_line_fore_local_payload(&local()).to_vec(),
        line(7).payload().to_vec(),
    ] {
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
fn document(projection: &[u8], body: &str, boot: &str, fore: &[u8], resource: &[u8]) -> Vec<u8> {
    let play = format!(
        "body-play/sha256:{}",
        body_play_digest(body, "wake", "body-plan", 1)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let (_, local_digest) = local_fragment_set_commitment(&["fragment"]).unwrap();
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
        fragment: "fragment",
        host: "host/1",
        boot,
        offer_generation: 1,
        local_fragment_count: 1,
        local_fragment_digest: local_digest,
        resource_bindings: resource,
        fore_ports: fore,
        projection_length: projection.len() as u32,
        projection_digest: Sha256::digest(projection).into(),
    };
    encode_body_fragment_document(&metadata, projection, 4096).unwrap()
}
fn inventory() -> Vec<u8> {
    [remote().payload().as_slice(), local().payload().as_slice()].concat()
}
fn slot(f: &Fixture) -> IssuedHostCommandChallenge<'_> {
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
fn command<'a>(f: &'a Fixture, payload: &'a [u8]) -> AttributedHostCommand<'a, 'a> {
    let slot = slot(f);
    let signature = f
        .key
        .sign(&host_command_attribution_transcript(slot.challenge(), payload).unwrap())
        .to_bytes();
    slot.authenticate(payload, &signature, 11).unwrap()
}
#[test]
fn explicit_profile_authenticates_mixed_schema8_with_canonical_metadata_inventory() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let (fragment, ports) = receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings),
    )
    .unwrap();
    assert_eq!(fragment.projection(), projection);
    assert_eq!(fragment.metadata().body, "body/1");
    assert_eq!(ports.as_slice(), [remote(), local[0]]);
    // The original legacy APIs still refuse schema 8, including the trusted
    // authoring ABI approval entrance. ABI approval cannot add Line pins.
    let mut old = profile(&local, &lines, &bindings).preparation;
    let fore = [remote(), local[0]];
    old.fore = Some(&fore);
    assert!(receive_delegated_body_fragment(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        old
    )
    .is_err());
    assert!(receive_delegated_body_fragment_with_approved_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings).preparation
    )
    .is_err());
}
#[test]
fn missing_delegation_bad_signature_stale_context_and_foreign_line_refuse() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    let local = [local()];
    let lines = [line(7)];
    let foreign = [line(8)];
    let bindings = [binding()];
    assert!(slot(&f).authenticate(&bytes, &[0; 64], 11).is_err());
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        None,
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    let mut stale = f.context();
    stale.now_millis = 30;
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        stale,
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    assert!(receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &foreign, &bindings)
    )
    .is_err());
}
#[test]
fn metadata_scope_inventory_capacity_resources_and_schema7_cannot_bypass_checks() {
    let f = Fixture::new();
    let projection = projection(c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA);
    let local = [local()];
    let lines = [line(7)];
    let bindings = [binding()];
    let reversed = [local[0].payload().as_slice(), remote().payload().as_slice()].concat();
    for bytes in [
        document(&projection, "body/other", "boot/1", &inventory(), &[]),
        document(&projection, "body/1", "boot/other", &inventory(), &[]),
        document(&projection, "body/1", "boot/1", &reversed, &[]),
        document(&projection, "body/1", "boot/1", &inventory(), &[255]),
    ] {
        assert!(receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        )
        .is_err());
    }
    let invalid_resources = document(&projection, "body/1", "boot/1", &inventory(), &[255]);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &invalid_resources),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Resources(_))
    ));
    let bytes = document(&projection, "body/1", "boot/1", &inventory(), &[]);
    assert!(receive_delegated_body_fragment_with_line_fore::<1>(
        Some(&policy()),
        f.context(),
        command(&f, &bytes),
        profile(&local, &lines, &bindings)
    )
    .is_err());
    let mut conflicting = profile(&local, &lines, &bindings);
    conflicting.preparation.fore = Some(&local);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            conflicting
        ),
        Err(DelegatedFragmentRefusal::ForeApproval)
    ));
    let narrow = HostPreparationDelegation {
        maximum_runtime_state_bytes: 64,
        ..policy()
    };
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&narrow),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Assignment(_))
    ));
    let schema7 = projection_with_schema7();
    let bytes = document(&schema7, "body/1", "boot/1", &inventory(), &[]);
    assert!(matches!(
        receive_delegated_body_fragment_with_line_fore::<2>(
            Some(&policy()),
            f.context(),
            command(&f, &bytes),
            profile(&local, &lines, &bindings)
        ),
        Err(DelegatedFragmentRefusal::Assignment(
            c::AssignedPlanRefusal::WrongSchema
        ))
    ));
}
fn projection_with_schema7() -> Vec<u8> {
    projection(c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA)
}
