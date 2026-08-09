use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, CapabilityId,
    HostOperationContractId, ResourceBinding, ResourceClassId, ResourcePoolId,
};
use conduit_net::{
    execute_fixture_join, network_capable_advertisement, network_join_offer,
    network_omitting_advertisement, NetworkAttachmentId, NetworkJoinError, NetworkJoinRequest,
    NETWORK_CONFIG_AUTHORITY, NETWORK_CONFIG_SUBJECT, NETWORK_JOIN_HOST_OPERATION,
    WIFI_STATION_RESOURCE_CLASS,
};

fn resource() -> ResourceBinding {
    ResourceBinding {
        pool_id: ResourcePoolId::from("resource/wifi-station-0"),
        class_id: ResourceClassId::from(WIFI_STATION_RESOURCE_CLASS),
        units: 1,
        protected: None,
        compute: None,
    }
}

fn grant(advertisement: &conduit_core::HostAdvertisement) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/network-config-once"),
        contract_id: AuthorityContractId::from(NETWORK_CONFIG_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(NETWORK_JOIN_HOST_OPERATION),
        subject_kind: kind_id(NETWORK_CONFIG_SUBJECT),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: CapabilityId::from("capability/network-join"),
    }
}

fn request<'a>(credential: &'a [u8]) -> NetworkJoinRequest<'a> {
    NetworkJoinRequest {
        ssid: b"fixture-lan",
        credential,
    }
}

#[test]
fn base_executes_with_exact_resource_authority_and_boot_scoped_attachment() {
    let advertisement = network_capable_advertisement("host/network", "boot/network-1");
    let attachment = execute_fixture_join(
        request(b"volatile-secret"),
        &advertisement,
        &CapabilityId::from("capability/network-join"),
        &resource(),
        Some(&grant(&advertisement)),
        NetworkAttachmentId::from("attachment/network-1"),
        1,
    )
    .unwrap();
    assert_eq!(attachment.host_id, advertisement.host_id);
    assert_eq!(attachment.boot_id, advertisement.boot_id);
    assert_eq!(attachment.interface_pool_id, resource().pool_id);
}

#[test]
fn support_resource_authority_and_boot_fail_independently() {
    let capable = network_capable_advertisement("host/network", "boot/network-1");
    let omitted = network_omitting_advertisement("host/minimal", "boot/minimal-1");
    let selected = CapabilityId::from("capability/network-join");
    let attachment = || NetworkAttachmentId::from("attachment/network-1");

    assert_eq!(
        execute_fixture_join(
            request(b"secret"),
            &omitted,
            &selected,
            &resource(),
            None,
            attachment(),
            1,
        ),
        Err(NetworkJoinError::Unsupported)
    );
    assert_eq!(
        execute_fixture_join(
            request(b"secret"),
            &capable,
            &selected,
            &ResourceBinding {
                pool_id: ResourcePoolId::from("resource/not-advertised"),
                ..resource()
            },
            Some(&grant(&capable)),
            attachment(),
            1,
        ),
        Err(NetworkJoinError::MissingResource)
    );
    assert_eq!(
        execute_fixture_join(
            request(b"secret"),
            &capable,
            &selected,
            &resource(),
            None,
            attachment(),
            1,
        ),
        Err(NetworkJoinError::MissingAuthority)
    );
    let mut stale = grant(&capable);
    stale.boot_id = conduit_core::BootId::from("boot/network-old");
    assert_eq!(
        execute_fixture_join(
            request(b"secret"),
            &capable,
            &selected,
            &resource(),
            Some(&stale),
            attachment(),
            1,
        ),
        Err(NetworkJoinError::StaleAuthority)
    );
}

#[test]
fn credential_bytes_never_enter_serialized_advertisement_or_attachment() {
    let advertisement = network_capable_advertisement("host/network", "boot/network-1");
    let attachment = execute_fixture_join(
        request(b"do-not-serialize-this-secret"),
        &advertisement,
        &CapabilityId::from("capability/network-join"),
        &resource(),
        Some(&grant(&advertisement)),
        NetworkAttachmentId::from("attachment/network-1"),
        1,
    )
    .unwrap();
    let report = serde_json::to_string(&(advertisement, attachment)).unwrap();
    assert!(!report.contains("do-not-serialize-this-secret"));
    assert!(!report.contains("fixture-lan"));
}

#[test]
fn equal_face_is_compatible_but_resource_and_authority_stay_exact() {
    let mut advertisement = network_capable_advertisement("host/network", "boot/network-1");
    let canonical = advertisement.capabilities[0].checked_face();
    advertisement.capabilities[0].kind_id = kind_id("vendor/associate-network");
    advertisement.capabilities[0].kind_contract_revision =
        conduit_core::KindContractRevision::from("vendor/associate-network@7");
    assert_eq!(advertisement.capabilities[0].checked_face(), canonical);
    assert_eq!(
        canonical,
        network_join_offer(
            CapabilityId::from("other/id"),
            conduit_core::ImplementationId::from("other/implementation"),
            conduit_core::ArtifactId::from("other/artifact"),
        )
        .checked_face()
    );

    let mut wrong = grant(&advertisement);
    wrong.capability_id = CapabilityId::from("capability/not-selected");
    assert_eq!(
        execute_fixture_join(
            request(b"secret"),
            &advertisement,
            &CapabilityId::from("capability/network-join"),
            &resource(),
            Some(&wrong),
            NetworkAttachmentId::from("attachment/network-1"),
            1,
        ),
        Err(NetworkJoinError::AuthorityMismatch)
    );
}

#[test]
fn credential_and_attachment_bounds_fail_closed() {
    let advertisement = network_capable_advertisement("host/network", "boot/network-1");
    let oversized = [7_u8; conduit_net::MAXIMUM_CREDENTIAL_BYTES + 1];
    assert_eq!(
        execute_fixture_join(
            request(&oversized),
            &advertisement,
            &CapabilityId::from("capability/network-join"),
            &resource(),
            Some(&grant(&advertisement)),
            NetworkAttachmentId::from("attachment/network-1"),
            1,
        ),
        Err(NetworkJoinError::CredentialTooLarge)
    );
}
