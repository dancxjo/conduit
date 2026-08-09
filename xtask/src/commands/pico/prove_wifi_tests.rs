use super::*;
use crate::commands::pico::firmware::{AssetEntry, GeneratedImageIdentity};

fn identity() -> FirmwareIdentity {
    FirmwareIdentity {
        schema: "conduit-pico-w-signal/identity@1".into(),
        git_revision: "revision".into(),
        target: "thumbv6m-none-eabi".into(),
        profile: "release".into(),
        firmware_mode: "wifi-bootstrap".into(),
        firmware_build_id: "build".into(),
        firmware_sha256: "sha".into(),
        generated_image: GeneratedImageIdentity {
            schema: "conduit.pico-network.generated-image@1".into(),
            firmware_mode: "wifi-bootstrap".into(),
            firmware_build_id: "build".into(),
            source_document_id: "source".into(),
            checked_form_id: "checked".into(),
            expanded_form_id: "expanded".into(),
            plan_id: "plan".into(),
            fragment_id: "fragment".into(),
            host_id: "host".into(),
            boot_id: "generated-boot".into(),
            active_play_id: "generated-play".into(),
            boot_clue_id: "boot-clue".into(),
            presentation_ids: vec![],
            presentation_clue_ids: vec![],
            terminal_clue_id: "attachment-clue".into(),
            offer_generation: 1,
            nodes: 1,
            cords: 1,
            host_operations: 1,
            cord_value_slots: 1,
            cord_value_bytes: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
            clue_items: 16,
            clue_bytes: 1024,
        },
        r1_control_images: None,
        cyw43_commit: "commit".into(),
        cyw43_assets: vec![AssetEntry {
            filename: "asset".into(),
            sha256: "sha".into(),
        }],
    }
}

#[test]
fn attachment_clue_requires_exact_runtime_and_generated_identities() {
    let identity = identity();
    let runtime = RuntimeTranscriptIdentity {
        boot_id: "runtime-boot".into(),
        active_play_id: "runtime-play".into(),
    };
    let record = serde_json::json!({
        "schema": "conduit.network/attachment-clue@1",
        "firmware_build_id": "build",
        "source_document_id": "source",
        "checked_form_id": "checked",
        "expanded_form_id": "expanded",
        "plan_id": "plan",
        "fragment_id": "fragment",
        "host_id": "host",
        "boot_id": "runtime-boot",
        "active_play_id": "runtime-play",
        "attachment_id": "attachment",
        "interface_pool_id": conduit_net::R1_WIFI_STATION_POOL_ID,
        "generation": 1,
        "clue_id": "attachment-clue"
    });
    assert!(verify_attachment_clue(&record.to_string(), &identity, &runtime).is_ok());

    let mut stale = record;
    stale["boot_id"] = "stale-boot".into();
    assert!(verify_attachment_clue(&stale.to_string(), &identity, &runtime).is_err());
}

#[test]
fn failure_clue_exposes_only_the_bounded_code() {
    let failure = serde_json::json!({
        "schema": "conduit.network/join-failure-clue@1",
        "firmware_build_id": "build",
        "source_document_id": "source",
        "checked_form_id": "checked",
        "expanded_form_id": "expanded",
        "plan_id": "plan",
        "fragment_id": "fragment",
        "host_id": "host",
        "boot_id": "runtime-boot",
        "active_play_id": "runtime-play",
        "interface_pool_id": conduit_net::R1_WIFI_STATION_POOL_ID,
        "clue_id": "attachment-clue",
        "error_code": "network-join-failed"
    });
    let error = verify_attachment_clue(
        &failure.to_string(),
        &identity(),
        &RuntimeTranscriptIdentity {
            boot_id: "runtime-boot".into(),
            active_play_id: "runtime-play".into(),
        },
    )
    .expect_err("failure Clue must fail the proof")
    .to_string();
    assert!(error.contains("network-join-failed"));
    assert!(!error.contains("ssid"));
    assert!(!error.contains("credential"));
}
