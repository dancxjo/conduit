use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};

#[test]
fn http_names_keep_friendly_families_separate_from_exact_contract_truth() {
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("identity-audit-host"),
            boot_id: BootId::from("identity-audit-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_http(),
    );
    let advertisement = host.advertisement();
    let client = advertisement
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .expect("HTTP client offer");

    for friendly in [
        client.implementation.execution_profile_id.as_str(),
        client.implementation.implementation_id.as_str(),
        client.implementation.artifact_id.as_str(),
        client.host_operations[0].contract_id.as_str(),
        client.resource_requirements[0].class_id.as_str(),
        client.authority_requirements[0].contract_id.as_str(),
    ] {
        assert!(!friendly.contains('@'), "decorative version in {friendly}");
    }
    assert_eq!(
        client.kind_contract_revision.as_str(),
        conduit_web::HTTP_CLIENT_REVISION
    );
    assert!(client.kind_contract_revision.as_str().ends_with("@1"));
    assert_eq!(client.host_operations[0].maximum_in_flight, 1);
    assert_eq!(client.resource_requirements[0].units, 1);
    assert_eq!(
        client.authority_requirements[0].host_operation_contract_id,
        client.host_operations[0].contract_id
    );
}

#[test]
#[cfg(all(target_os = "linux", feature = "isolated-http-base"))]
fn isolated_provider_keeps_real_wire_dispatch_independent() {
    assert_eq!(conduit_std_host::isolated_http_base::PROTOCOL_VERSION, 1);
    assert!(!conduit_std_host::isolated_http_base::ISOLATED_HTTP_IMPLEMENTATION.contains('@'));
    assert!(!conduit_std_host::isolated_http_base::ISOLATED_HTTP_PROFILE.contains('@'));
    assert!(!conduit_std_host::isolated_http_base::ISOLATED_HTTP_ARTIFACT.contains('@'));
}
