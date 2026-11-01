use super::*;
use crate::body_owner::continuing::native::{CompletedScope, SourceSelection};
use std::sync::{Arc, mpsc};
fn value(sequence: u64) -> LiveFore {
    let selection = Arc::new(SourceSelection {
        attachment_id: "green".into(),
        host_id: "host/refused".into(),
        boot_id: "boot/refused".into(),
        capability_id: "cap/source".into(),
        provider_instance_id: "provider/source".into(),
        provider_generation: 1,
        plan_id: "plan/refused".into(),
        fragment_id: "fragment/refused".into(),
        placement_id: "placement/source".into(),
        configuration_id: "configuration/source".into(),
        firmware_sha256: [7; 32],
    });
    let native_play = conduit_core::bind_active_play(
        &selection.plan_id,
        &selection.host_id,
        &selection.boot_id,
        4,
    );
    let mut bytes = [0; conduit_environment_measurement::source_observation::MAXIMUM_BYTES];
    bytes[..3].copy_from_slice(&[sequence as u8, 17, 255]);
    LiveFore {
        scope: Arc::new(CompletedScope {
            selection,
            native_play,
        }),
        sequence,
        length: 3,
        bytes,
    }
}
#[test]
fn rejected_source_fore_retains_first_and_quiescent_trailing_without_authority() {
    let mut custody = ForeRefusal::new(value(16), "finite stage full".into());
    let (sender, receiver) = mpsc::sync_channel(1);
    sender.send(value(17)).unwrap();
    drop(sender);
    custody.capture_trailing(&receiver);
    let document = custody.diagnostics();
    assert_eq!(document["first"]["sequence"], 16);
    assert_eq!(document["first"]["canonical_hex"], "1011ff");
    assert_eq!(document["trailing"]["sequence"], 17);
    assert_eq!(document["trailing"]["canonical_hex"], "1111ff");
    assert_eq!(
        document["first"]["source_selection"]["host_id"],
        "host/refused"
    );
    assert_eq!(document["producer_retirement_authorized"], false);
    assert_eq!(document["durable_recording_acknowledged"], false);
    assert!(document.get("credential").is_none());
    assert!(document.get("grant").is_none());
    custody.capture_trailing(&receiver);
    assert_eq!(custody.diagnostics(), document);
}
