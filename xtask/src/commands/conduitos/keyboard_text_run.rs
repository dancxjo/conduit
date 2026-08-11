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
    Ok(())
}

fn is_identity(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
