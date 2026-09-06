use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, CapabilityId,
    HostOperationContractId, ResourceBinding, ResourceClassId, ResourcePoolId,
};
use conduit_net::{
    decode_network_attachment, decode_network_join_request, encode_network_attachment,
    encode_network_join_request, execute_fixture_join, network_capable_advertisement,
    network_join_offer, network_omitting_advertisement, NetworkAttachmentId, NetworkAttachmentInfo,
    NetworkJoinError, NetworkJoinRequest, MAXIMUM_JOIN_OUTPUT_BYTES, NETWORK_CONFIG_AUTHORITY,
    NETWORK_CONFIG_SUBJECT, NETWORK_JOIN_HOST_OPERATION, WIFI_STATION_RESOURCE_CLASS,
};

fn resource() -> ResourceBinding {
    ResourceBinding {
        content: None,
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
fn boot_scoped_attachment_info_round_trips_through_one_bounded_value() {
    let attachment = NetworkAttachmentInfo {
        attachment_id: "r1/pico-network-attachment-1",
        host_id: "r1/pico-w",
        boot_id:
            "conduit-pico-w-signal/runtime-boot:0000000000000000:00000000000000000000000000000000",
        interface_pool_id: "r1/pico-wifi-station-0",
        generation: 1,
    };
    let mut encoded = [0_u8; MAXIMUM_JOIN_OUTPUT_BYTES as usize];
    let encoded_len = encode_network_attachment(attachment, &mut encoded).unwrap();
    assert_eq!(
        decode_network_attachment(&encoded[..encoded_len]).unwrap(),
        attachment
    );
    assert!(encoded_len <= MAXIMUM_JOIN_OUTPUT_BYTES as usize);
    assert_eq!(
        decode_network_attachment(&encoded[..encoded_len - 1]),
        Err(NetworkJoinError::InvalidAttachment)
    );
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
                content: None,
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

#[test]
fn volatile_join_info_round_trips_only_through_an_admitted_buffer() {
    let secret = b"do-not-render-or-retain";
    let mut encoded = [0_u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let encoded_len = encode_network_join_request(request(secret), &mut encoded).unwrap();
    let decoded = decode_network_join_request(&encoded[..encoded_len]).unwrap();
    assert_eq!(decoded.ssid, b"fixture-lan");
    assert_eq!(decoded.credential, secret);
    assert_eq!(encoded_len, 7 + b"fixture-lan".len() + secret.len());
}

#[test]
fn malformed_trailing_invalid_utf8_and_small_output_fail_distinctly() {
    let mut encoded = [0_u8; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let encoded_len = encode_network_join_request(request(b"secret"), &mut encoded).unwrap();
    encoded[encoded_len] = 0;
    assert!(matches!(
        decode_network_join_request(&encoded[..=encoded_len]),
        Err(NetworkJoinError::MalformedRequest)
    ));

    let invalid_ssid = NetworkJoinRequest {
        ssid: &[0xff],
        credential: b"secret",
    };
    assert_eq!(
        encode_network_join_request(invalid_ssid, &mut encoded),
        Err(NetworkJoinError::MalformedRequest)
    );
    assert_eq!(
        encode_network_join_request(request(b"secret"), &mut [0_u8; 4]),
        Err(NetworkJoinError::OutputTooSmall)
    );
}

#[test]
#[cfg(feature = "form-catalog")]
fn canonical_bootstrap_form_carries_no_credentials_or_platform_facts() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_network_bootstrap_catalogs(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/r1-network-bootstrap/main.conduit");
    let document = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&document, &startup).unwrap();
    assert_eq!(checked.forms.len(), 1);
    let form =
        conduit_form::expand_canonical_form(&checked, "r1-network-bootstrap", &profile).unwrap();
    assert_eq!(form.gears.len(), 3);
    assert_eq!(form.connections.len(), 2);
    assert!(
        form.connections
            .iter()
            .any(|connection| connection.value_kind.as_str()
                == conduit_net::NETWORK_JOIN_REQUEST_KIND)
    );
    assert!(form
        .connections
        .iter()
        .any(|connection| connection.value_kind.as_str() == conduit_net::NETWORK_ATTACHMENT_KIND));
    for forbidden in [
        "ssid",
        "password",
        "UsbCdc",
        "wifi-station",
        "HostId",
        "BootId",
    ] {
        assert!(!source.contains(forbidden));
    }
}
