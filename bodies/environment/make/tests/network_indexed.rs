//! Same local task planner, current offers and authority; no radio effects.
extern crate alloc;
#[path = "../src/network_planning.rs"]
#[allow(dead_code)]
mod planning;
use conduit_core as c;
use conduit_net::*;
use conduit_network_join_call::{installation::*, *};
struct Inert;
impl NetworkJoinProvider for Inert {
    async fn join(
        &mut self,
        _: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        panic!("planning has no radio effects")
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        panic!("planning has no cleanup effects")
    }
}
impl CredentialSource for Inert {
    fn take_acknowledged(&mut self, _: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        panic!("planning cannot consume secrets")
    }
}
fn identity(role: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "fixture/host".into(),
        boot: "fixture/boot".into(),
        base: format!("fixture/base/{role}").into(),
        instance: format!("fixture/instance/{role}").into(),
        provider_generation: 1,
        resource: format!("fixture/resource/{role}").into(),
        resource_generation: c::ResourceGenerationId(format!("fixture/generation/{role}")),
        envelope: format!("fixture/envelope/{role}").into(),
        artifact: format!("fixture/artifact/{role}").into(),
        implementation: format!("fixture/implementation/{role}").into(),
        base_implementation: format!("fixture/base-implementation/{role}").into(),
    }
}
#[test]
fn indexed_task_planning_matches_owned_plan_and_preserves_refusals() {
    let source = conduit_environment_make::compile_network_setup().unwrap();
    let expected = c::PlotIdentity {
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
    };
    let bytes = conduit_plot::flat_image::indexed::encode(&source).unwrap();
    // SAFETY: this fixture just checked and encoded the complete Source.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/network".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    // SAFETY: inert exclusively owned CPU fixtures; effects panic above.
    let (join, credentials) = unsafe {
        (
            ReadyNetworkJoinBase::new(identity("join"), 1, Inert).unwrap(),
            credential_installation::ReadyCredentialBase::new(identity("credentials"), Inert)
                .unwrap(),
        )
    };
    join.append_to_advertisement(&mut host).unwrap();
    credentials.append_to_advertisement(&mut host).unwrap();
    host.capabilities.push(network_attachment_sign_local_offer(
        "fixture/sign".into(),
        "fixture/sign-implementation".into(),
        "fixture/artifact".into(),
    ));
    host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let grants = [
        c::AuthorityGrant {
            grant_id: "grant/credentials".into(),
            contract_id: NETWORK_CREDENTIALS_AUTHORITY.into(),
            host_call_contract_id: NETWORK_CREDENTIALS_HOST_CALL.into(),
            subject_kind: NETWORK_CREDENTIALS_SUBJECT.into(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: credential_installation::CREDENTIAL_CAPABILITY.into(),
        },
        c::AuthorityGrant {
            grant_id: "grant/join".into(),
            contract_id: NETWORK_CONFIG_AUTHORITY.into(),
            host_call_contract_id: NETWORK_JOIN_HOST_CALL.into(),
            subject_kind: NETWORK_CONFIG_SUBJECT.into(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: JOIN_CAPABILITY.into(),
        },
    ];
    let bases = [
        "conduit.base/local@1".into(),
        "fixture/base-implementation/join".into(),
        "fixture/base-implementation/credentials".into(),
    ];
    let owned = planning::PreparedNetworkSetup::from_checked_for_identity(
        &expected, &source, &host, &bases, &grants,
    )
    .unwrap();
    let indexed = planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected, &image, &host, &bases, &grants,
    )
    .unwrap();
    assert_eq!(indexed.plan(), owned.plan());
    assert_eq!(indexed.lowered(), owned.lowered());
    assert!(c::verify_plan(indexed.plan()));
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected,
        &image,
        &host,
        &bases,
        &[]
    )
    .is_err());
    let mut wrong = expected.clone();
    wrong.expanded_plot_id = "wrong/source".into();
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &wrong, &image, &host, &bases, &grants
    )
    .is_err());
    host.capabilities
        .retain(|offer| offer.capability_id.as_str() != JOIN_CAPABILITY);
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected, &image, &host, &bases, &grants
    )
    .is_err());
}
