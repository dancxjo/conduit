use super::*;
use std::io::Cursor;

fn expected_identity() -> GeneratedImageIdentity {
    GeneratedImageIdentity {
        schema: "conduit.pico-signal.generated-image@1".into(),
        firmware_mode: "pico-local".into(),
        firmware_build_id: "firmware-build".into(),
        source_document_id: "source".into(),
        checked_form_id: "checked".into(),
        expanded_form_id: "expanded".into(),
        plan_id: "plan".into(),
        fragment_id: "fragment".into(),
        host_id: "host".into(),
        boot_id: "boot".into(),
        active_play_id: "play".into(),
        boot_sign_id: "boot-sign".into(),
        presentation_ids: (0..EXPECTED_RECEIPTS)
            .map(|sequence| format!("presentation-{sequence}"))
            .collect(),
        presentation_sign_ids: (0..EXPECTED_RECEIPTS)
            .map(|sequence| format!("sign-{sequence}"))
            .collect(),
        terminal_sign_id: "terminal-sign".into(),
        offer_generation: 1,
        nodes: 2,
        cords: 1,
        host_operations: 2,
        cord_value_slots: 1,
        cord_value_bytes: 9,
        sign_items: 7,
        sign_bytes: 327,
    }
}

fn expected_firmware_identity() -> FirmwareIdentity {
    FirmwareIdentity {
        schema: "conduit-pico-w-signal/identity@1".into(),
        git_revision: "revision".into(),
        target: "thumbv6m-none-eabi".into(),
        profile: "release".into(),
        firmware_mode: "pico-local".into(),
        firmware_build_id: "firmware-build".into(),
        firmware_sha256: "sha256".into(),
        generated_image: expected_identity(),
        r1_control_images: None,
        cyw43_commit: "commit".into(),
        cyw43_assets: Vec::new(),
    }
}

fn boot() -> String {
    format!(
        concat!(
            "{{\"schema\":\"conduit-pico-w-signal/boot@1\",",
            "\"firmware_build_id\":\"firmware-build\",",
            "\"source_document_id\":\"source\",",
            "\"checked_form_id\":\"checked\",",
            "\"expanded_form_id\":\"expanded\",",
            "\"plan_id\":\"plan\",",
            "\"fragment_id\":\"fragment\",",
            "\"host_id\":\"host\",",
            "\"boot_id\":\"boot\",",
            "\"runtime_boot_id\":\"runtime-boot\",",
            "\"runtime_active_play_id\":\"{}\",",
            "\"sign_id\":\"boot-sign\"}}\n"
        ),
        runtime_play(),
    )
}

fn receipt(sequence: usize) -> String {
    format!(
        concat!(
            "{{\"schema\":\"conduit-pico-w-signal/receipt@1\",",
            "\"firmware_build_id\":\"firmware-build\",",
            "\"source_document_id\":\"source\",",
            "\"checked_form_id\":\"checked\",",
            "\"expanded_form_id\":\"expanded\",",
            "\"plan_id\":\"plan\",",
            "\"fragment_id\":\"fragment\",",
            "\"host_id\":\"host\",",
            "\"boot_id\":\"boot\",",
            "\"active_play_id\":\"play\",",
            "\"runtime_boot_id\":\"runtime-boot\",",
            "\"runtime_active_play_id\":\"{}\",",
            "\"sequence\":{},",
            "\"level\":{},",
            "\"presentation_id\":\"presentation-{}\",",
            "\"sign_id\":\"sign-{}\"}}\n"
        ),
        runtime_play(),
        sequence,
        sequence % 2 == 1,
        sequence,
        sequence,
    )
}

fn terminal() -> String {
    format!(
        concat!(
            "{{\"schema\":\"conduit-pico-w-signal/terminal@1\",",
            "\"firmware_build_id\":\"firmware-build\",",
            "\"source_document_id\":\"source\",",
            "\"checked_form_id\":\"checked\",",
            "\"expanded_form_id\":\"expanded\",",
            "\"plan_id\":\"plan\",",
            "\"fragment_id\":\"fragment\",",
            "\"host_id\":\"host\",",
            "\"boot_id\":\"boot\",",
            "\"active_play_id\":\"play\",",
            "\"runtime_boot_id\":\"runtime-boot\",",
            "\"runtime_active_play_id\":\"{}\",",
            "\"success\":true,",
            "\"sign_id\":\"terminal-sign\"}}\n"
        ),
        runtime_play(),
    )
}

fn runtime_play() -> String {
    conduit_core::bind_active_play(
        &conduit_core::PlanId::from("plan"),
        &conduit_core::HostId::from("host"),
        &conduit_core::BootId::from("runtime-boot"),
        0,
    )
    .active_play_id
    .as_str()
    .to_owned()
}

#[test]
fn accepts_exact_sixteen_receipts_and_terminal() {
    let mut input = String::new();
    input.push_str(&boot());
    for sequence in 0..EXPECTED_RECEIPTS {
        input.push_str(&receipt(sequence));
    }
    input.push_str(&terminal());
    verify_receipts(Cursor::new(input), &expected_firmware_identity())
        .expect("valid receipt stream");
}

#[test]
fn rejects_reordered_receipt() {
    let input = format!("{}{}{}", boot(), receipt(1), terminal());
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}

#[test]
fn rejects_mutated_identity_field() {
    let mut input = String::new();
    input.push_str(&boot());
    for sequence in 0..EXPECTED_RECEIPTS {
        input.push_str(&receipt(sequence));
    }
    input.push_str(&terminal().replace("\"plan_id\":\"plan\"", "\"plan_id\":\"mutated\""));
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}

#[test]
fn rejects_mutated_firmware_build_identity() {
    let mut input = String::new();
    input.push_str(&boot());
    for sequence in 0..EXPECTED_RECEIPTS {
        input.push_str(&receipt(sequence));
    }
    input.push_str(&terminal().replace(
        "\"firmware_build_id\":\"firmware-build\"",
        "\"firmware_build_id\":\"other-build\"",
    ));
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}

#[test]
fn rejects_missing_runtime_identity_field() {
    let input = format!(
        "{}{}{}",
        boot().replace("\"runtime_boot_id\":\"runtime-boot\",", ""),
        (0..EXPECTED_RECEIPTS).map(receipt).collect::<String>(),
        terminal()
    );
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}

#[test]
fn rejects_runtime_identity_reusing_planned_identity() {
    let input = format!(
        "{}{}{}",
        boot()
            .replace(
                "\"runtime_boot_id\":\"runtime-boot\"",
                "\"runtime_boot_id\":\"boot\"",
            )
            .replace(
                &format!("\"runtime_active_play_id\":\"{}\"", runtime_play()),
                "\"runtime_active_play_id\":\"play\""
            ),
        (0..EXPECTED_RECEIPTS).map(receipt).collect::<String>(),
        terminal()
    );
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}

#[test]
fn rejects_runtime_identity_change_after_boot() {
    let mut input = String::new();
    input.push_str(&boot());
    for sequence in 0..EXPECTED_RECEIPTS {
        input.push_str(&receipt(sequence));
    }
    input.push_str(&terminal().replace(
        &format!("\"runtime_active_play_id\":\"{}\"", runtime_play()),
        "\"runtime_active_play_id\":\"other-runtime-play\"",
    ));
    assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
}
