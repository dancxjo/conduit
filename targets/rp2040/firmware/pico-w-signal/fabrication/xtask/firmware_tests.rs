use conduit_system_continuity::R1SignalRouteSet;

use super::appliance_identity::{
    ApplianceFirmwareIdentity, ApplianceGeneratedImageIdentity, ApplianceHilClientFirmwareIdentity,
    ApplianceHilClientGeneratedImageIdentity, APPLIANCE_HIL_CLIENT_ARTIFACT,
};
use super::doctor::{CYW43_ASSETS, CYW43_COMMIT};
use super::firmware::{AssetEntry, FirmwareIdentity, GeneratedImageIdentity, R1ControlImageFamily};

fn control_image(routes: R1SignalRouteSet) -> GeneratedImageIdentity {
    let exact = conduit_system_continuity::exact_r1_control_plan(
        conduit_core::BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
        routes,
    )
    .unwrap();
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| {
            fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
        })
        .unwrap();
    GeneratedImageIdentity {
        schema: "conduit.pico-signal.generated-image@1".into(),
        firmware_mode: "r1-control".into(),
        firmware_build_id: format!("control-build-{}", exact.plan.plan_id.as_str()),
        source_document_id: exact.plan.source_document_id.as_str().into(),
        checked_form_id: exact.plan.checked_form_id.as_str().into(),
        expanded_form_id: exact.plan.expanded_form_id.as_str().into(),
        plan_id: exact.plan.plan_id.as_str().into(),
        fragment_id: fragment.fragment_id.as_str().into(),
        host_id: conduit_r1_network_conformance::R1_PICO_HOST_ID.into(),
        boot_id: conduit_r1_network_conformance::R1_PICO_BOOT_ID.into(),
        active_play_id: "control-play".into(),
        boot_sign_id: "control-boot-sign".into(),
        presentation_ids: vec![],
        presentation_sign_ids: vec![],
        terminal_sign_id: "control-terminal-sign".into(),
        offer_generation: 1,
        nodes: 1,
        cords: 1,
        host_operations: 1,
        cord_value_slots: 1,
        cord_value_bytes: conduit_signal::SIGNAL_ENCODED_LEN,
        sign_items: 1,
        sign_bytes: 1,
    }
}

fn composite_identity() -> FirmwareIdentity {
    FirmwareIdentity {
        schema: "conduit-pico-w-signal/identity@2".into(),
        git_revision: "revision".into(),
        target: "thumbv6m-none-eabi".into(),
        profile: "release".into(),
        firmware_mode: "r1-control".into(),
        firmware_build_id: "network-build".into(),
        firmware_sha256: "sha".into(),
        generated_image: GeneratedImageIdentity {
            schema: "conduit.pico-network.generated-image@1".into(),
            firmware_mode: "r1-control".into(),
            firmware_build_id: "network-build".into(),
            source_document_id: "network-source".into(),
            checked_form_id: "network-checked".into(),
            expanded_form_id: "network-expanded".into(),
            plan_id: "network-plan".into(),
            fragment_id: "network-fragment".into(),
            host_id: conduit_r1_network_conformance::R1_PICO_HOST_ID.into(),
            boot_id: conduit_r1_network_conformance::R1_PICO_BOOT_ID.into(),
            active_play_id: "network-play".into(),
            boot_sign_id: "network-boot-sign".into(),
            presentation_ids: vec![],
            presentation_sign_ids: vec![],
            terminal_sign_id: "network-terminal-sign".into(),
            offer_generation: 1,
            nodes: 2,
            cords: 2,
            host_operations: 2,
            cord_value_slots: 2,
            cord_value_bytes: 334,
            sign_items: 1,
            sign_bytes: 1,
        },
        r1_control_images: Some(R1ControlImageFamily {
            plan_a: control_image(R1SignalRouteSet::WebSocketOnly),
            plan_b: control_image(R1SignalRouteSet::UsbOnly),
            plan_c: control_image(R1SignalRouteSet::WebSocketThenUsb),
        }),
        cyw43_commit: "commit".into(),
        cyw43_assets: vec![],
    }
}

#[test]
fn composite_manifest_requires_the_exact_ordered_control_family() {
    let identity = composite_identity();
    assert!(identity.verified_r1_control_images().is_ok());
    let plan_b = conduit_system_continuity::exact_r1_control_plan(
        conduit_core::BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
        R1SignalRouteSet::UsbOnly,
    )
    .unwrap()
    .plan;
    assert_eq!(
        identity
            .verified_r1_control_image(&plan_b.plan_id)
            .unwrap()
            .plan_id,
        plan_b.plan_id.as_str()
    );
    assert!(identity
        .verified_r1_control_image(&conduit_core::PlanId::from("not-sealed"))
        .is_err());

    let mut substituted = identity.clone();
    substituted.r1_control_images.as_mut().unwrap().plan_b = substituted
        .r1_control_images
        .as_ref()
        .unwrap()
        .plan_a
        .clone();
    assert!(substituted.verified_r1_control_images().is_err());

    let mut wrong_primary = identity;
    wrong_primary.generated_image.schema = "conduit.pico-signal.generated-image@1".into();
    assert!(wrong_primary.verified_r1_control_images().is_err());
}

fn appliance_identity() -> ApplianceFirmwareIdentity {
    let advertisement = conduit_rp2040_network_realization::pico_appliance_advertisement(
        "pico/appliance-hello",
        "image/boot-bound-at-runtime",
        conduit_rp2040_network_realization::PicoApplianceComposition::Hello,
        conduit_rp2040_network_realization::PicoApplianceInitialization::hello_ready(),
    )
    .unwrap();
    ApplianceFirmwareIdentity {
        schema: "conduit-pico-w-signal/appliance-identity@1".into(),
        git_revision: "revision".into(),
        target: "thumbv6m-none-eabi".into(),
        profile: "release".into(),
        firmware_mode: "appliance-hello".into(),
        firmware_build_id: "appliance-build".into(),
        firmware_sha256: "a".repeat(64),
        appliance_image: ApplianceGeneratedImageIdentity {
            schema: "conduit.pico-appliance/generated-image@1".into(),
            firmware_mode: "appliance-hello".into(),
            firmware_build_id: "appliance-build".into(),
            image_artifact: conduit_rp2040_network_realization::PICO_APPLIANCE_ARTIFACT.into(),
            service_artifacts: [
                conduit_rp2040_network_realization::AP_SERVICE_ARTIFACT,
                conduit_rp2040_network_realization::DHCP_SERVICE_ARTIFACT,
                conduit_rp2040_network_realization::DNS_SERVICE_ARTIFACT,
                conduit_rp2040_network_realization::HTTP_SERVICE_ARTIFACT,
            ]
            .map(str::to_owned)
            .to_vec(),
            host_advertisement: advertisement,
            ssid: conduit_rp2040_network_realization::APPLIANCE_SSID.into(),
            open_ap: true,
            channel: 6,
            server_address: conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS,
            local_name: conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME.into(),
            hello_body: conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY.into(),
            maximum_associations: conduit_rp2040_network_realization::MAXIMUM_AP_ASSOCIATIONS,
            maximum_dhcp_leases: conduit_rp2040_network_realization::MAXIMUM_DHCP_LEASES,
            maximum_dhcp_packet_bytes:
                conduit_rp2040_network_realization::MAXIMUM_DHCP_PACKET_BYTES,
            maximum_dns_packet_bytes: conduit_rp2040_network_realization::MAXIMUM_DNS_PACKET_BYTES,
            maximum_http_request_bytes:
                conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES,
            maximum_http_response_bytes:
                conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES,
            maximum_signs: conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_SIGNS,
            maximum_network_sockets:
                conduit_rp2040_network_realization::MAXIMUM_APPLIANCE_NETWORK_SOCKETS,
        },
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets: CYW43_ASSETS
            .iter()
            .map(|(filename, sha256)| AssetEntry {
                filename: (*filename).into(),
                sha256: (*sha256).into(),
            })
            .collect(),
    }
}

#[test]
fn appliance_manifest_seals_radio_service_offer_and_bounds() {
    let identity = appliance_identity();
    identity.verify().unwrap();

    let mut wrong_radio = identity.clone();
    wrong_radio.cyw43_assets[0].sha256 = "0".repeat(64);
    assert!(wrong_radio.verify().is_err());

    let mut wrong_service = identity.clone();
    wrong_service.appliance_image.service_artifacts.swap(0, 1);
    assert!(wrong_service.verify().is_err());

    let mut invented_offer = identity.clone();
    invented_offer
        .appliance_image
        .host_advertisement
        .capabilities
        .pop();
    assert!(invented_offer.verify().is_err());

    let mut enlarged_bound = identity;
    enlarged_bound.appliance_image.maximum_dhcp_leases += 1;
    assert!(enlarged_bound.verify().is_err());
}

#[test]
fn appliance_hil_client_manifest_is_fixture_only_and_exact() {
    let mut identity = ApplianceHilClientFirmwareIdentity {
        schema: "conduit-pico-w-signal/appliance-hil-client-identity@1".into(),
        git_revision: "revision".into(),
        target: "thumbv6m-none-eabi".into(),
        profile: "release".into(),
        firmware_mode: "appliance-hil-client".into(),
        firmware_build_id: "client-build".into(),
        firmware_sha256: "a".repeat(64),
        client_image: ApplianceHilClientGeneratedImageIdentity {
            schema: "conduit.pico-appliance/hil-client-image@1".into(),
            firmware_mode: "appliance-hil-client".into(),
            firmware_build_id: "client-build".into(),
            image_artifact: APPLIANCE_HIL_CLIENT_ARTIFACT.into(),
            fixture_only: true,
            usb_serial: "conduit-pico-hil-client".into(),
            ssid: conduit_rp2040_network_realization::APPLIANCE_SSID.into(),
            open_ap: true,
            server_address: conduit_rp2040_network_realization::DHCP_SERVER_ADDRESS,
            local_name: conduit_rp2040_network_realization::APPLIANCE_LOCAL_NAME.into(),
            hello_body: conduit_rp2040_network_realization::APPLIANCE_HELLO_BODY.into(),
            maximum_http_request_bytes:
                conduit_rp2040_network_realization::MAXIMUM_HTTP_REQUEST_BYTES,
            maximum_http_response_bytes:
                conduit_rp2040_network_realization::MAXIMUM_HTTP_RESPONSE_BYTES,
        },
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets: CYW43_ASSETS
            .iter()
            .map(|(filename, sha256)| AssetEntry {
                filename: (*filename).into(),
                sha256: (*sha256).into(),
            })
            .collect(),
    };
    identity.verify().unwrap();

    identity.client_image.fixture_only = false;
    assert!(identity.verify().is_err());
}
