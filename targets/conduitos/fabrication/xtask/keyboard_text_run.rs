//! Exact extraction and validation of the ordinary keyboard-text Form proof.

use super::{
    report::{GuestBootSign, GuestKeyboardSign, GuestKeyboardTextSign},
    ConduitosError,
};

pub(super) fn extract(serial: &str) -> Result<GuestKeyboardTextSign, ConduitosError> {
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KEYBOARD_TEXT_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-keyboard-text-sign",
            format!(
                "expected one structured keyboard-text Sign, found {}",
                signs.len()
            ),
        ));
    }
    serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-keyboard-text-sign", error.to_string()))
}

pub(super) fn validate(
    serial: &str,
    boot: &GuestBootSign,
    keyboard: &GuestKeyboardSign,
    sign: &GuestKeyboardTextSign,
    observatory: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    let ascii: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KEYBOARD_TEXT_PRESENT "))
        .collect();
    let unicode: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KEYBOARD_UNICODE_PRESENT "))
        .collect();
    let expected = ["H", "E", "L", "L", "O", "Æ", "É", "Λ"];
    if sign.schema != "conduit.conduitos.keyboard-text-form/v1"
        || sign.status != "completed"
        || sign.proof_class != "freestanding-emulator"
        || sign.host_id != keyboard.host_id
        || sign.boot_id != boot.boot_id
        || sign.form_machine_facts
        || sign.keymap_configuration != "conduit-intl"
        || sign.physical_transition_count != 38
        || sign
            .presentation_fragments
            .iter()
            .map(String::as_str)
            .ne(expected)
        || sign.visible_ascii != "HELLO"
        || !sign.bounded
        || !sign.completed
        || ascii != ["HELLO"]
        || unicode != ["ÆÉΛ"]
        || !is_identity(&sign.source_document_id)
        || !is_identity(&sign.checked_form_id)
        || !is_identity(&sign.expanded_form_id)
        || !is_identity(&sign.plan_id)
        || !is_identity(&sign.active_play_id)
    {
        return Err(ConduitosError::refusal(
            "invalid-keyboard-text-sign",
            format!("keyboard-text Sign failed exact validation: {sign:?}"),
        ));
    }
    validate_observatory(keyboard, sign, observatory)?;
    Ok(())
}

fn validate_observatory(
    keyboard: &GuestKeyboardSign,
    sign: &GuestKeyboardTextSign,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    conduit_observatory::validate_snapshot(snapshot)
        .map_err(|error| ConduitosError::refusal("invalid-keyboard-text-observatory", error))?;
    let expected_kinds = [
        conduit_semantic_catalog::KEYBOARD_KIND,
        conduit_semantic_catalog::KEYMAP_KIND,
        conduit_text::TEXT_UPPER_KIND,
        conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
    ];
    let Some(plan) = snapshot.plans.first() else {
        return Err(ConduitosError::refusal(
            "invalid-keyboard-text-observatory",
            "missing exact K6 Plan",
        ));
    };
    if snapshot.plans.len() != 1
        || snapshot.plays.len() != 1
        || plan.plan_id.as_str() != sign.plan_id
        || plan.source_document_id.as_str() != sign.source_document_id
        || plan.checked_form_id.as_str() != sign.checked_form_id
        || plan.expanded_form_id.as_str() != sign.expanded_form_id
        || snapshot.plays[0].active_play_id.as_str() != sign.active_play_id
        || snapshot.plays[0].terminal_disposition
            != Some(conduit_core::TerminalDisposition::Completed)
        || expected_kinds.iter().any(|expected| {
            !plan.fragments[0]
                .placements
                .iter()
                .any(|placement| placement.kind_id.as_str() == *expected)
        })
        || plan.fragments[0].connections.len() != 3
        || !snapshot
            .bases
            .iter()
            .any(|base| base.base_id.as_str() == keyboard.controller_base_id)
    {
        return Err(ConduitosError::refusal(
            "invalid-keyboard-text-observatory",
            "K6 semantic topology, realization, or identity chain disagreed",
        ));
    }
    let mut patchbay = patchbay_model::PatchbayTopology::new(1).map_err(|error| {
        ConduitosError::refusal("keyboard-text-patchbay-refused", error.to_string())
    })?;
    patchbay.ingest(snapshot).map_err(|error| {
        ConduitosError::refusal("keyboard-text-patchbay-refused", error.to_string())
    })?;
    let document = patchbay.document(None).map_err(|error| {
        ConduitosError::refusal("keyboard-text-patchbay-refused", error.to_string())
    })?;
    let rendered = document.lines().join("\n");
    for fact in [
        "kind=input/keyboard",
        "kind=input/keymap",
        "kind=text/upper",
        "kind=presentation/text",
        "implementation=conduitos/usb-hid-keyboard@1",
        "info=input/key-event@1",
        "conduitos.base/xhci@1",
    ] {
        if !rendered.contains(fact) {
            return Err(ConduitosError::refusal(
                "keyboard-text-patchbay-incomplete",
                format!("ordinary Patchbay projection omitted {fact}"),
            ));
        }
    }
    Ok(())
}

fn is_identity(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
