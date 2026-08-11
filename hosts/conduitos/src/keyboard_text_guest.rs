//! Freestanding presentation of the reviewed keyboard-text Plays.

use alloc::format;
use conduit_core::KeyEvent;

use crate::{
    arch, boot::BootRecord, identity, keyboard_text_observatory, keyboard_text_plan,
    keyboard_text_play, offer::HostOffer,
};

pub const PHYSICAL_TRANSITIONS: usize = 38;

pub fn run_reviewed_sequences(
    record: &BootRecord,
    identities: &identity::BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
    image_id: &str,
    events: &[KeyEvent; PHYSICAL_TRANSITIONS],
    framebuffer: Option<&conduit_observatory::FramebufferBasis>,
) -> Result<(), &'static str> {
    let prepared = prepare(identities, offer, build_id)?;
    arch::early_write(b"CONDUIT_KEYBOARD_TEXT_PRESENT ");
    let mut presented = 0usize;
    let report = keyboard_text_play::run_with_presentation(&prepared, events, |fragment| {
        if presented == 5 {
            arch::early_write(b"\nCONDUIT_KEYBOARD_UNICODE_PRESENT ");
        }
        arch::early_write(fragment.as_bytes());
        presented += 1;
    })
    .map_err(|_| "keyboard-text-play-refused")?;
    arch::early_write(b"\n");

    require(
        &report,
        &[
            b"H",
            b"E",
            b"L",
            b"L",
            b"O",
            "Æ".as_bytes(),
            "É".as_bytes(),
            "Λ".as_bytes(),
        ],
    )?;
    let sign = format!(
        "CONDUIT_KEYBOARD_TEXT_SIGN {{\"schema\":\"conduit.conduitos.keyboard-text-form/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"form_machine_facts\":false,\"keymap_configuration\":\"conduit-intl\",\"physical_transition_count\":{},\"presentation_fragments\":[\"H\",\"E\",\"L\",\"L\",\"O\",\"Æ\",\"É\",\"Λ\"],\"visible_ascii\":\"HELLO\",\"bounded\":true,\"completed\":true}}\n",
        prepared.source_document_id.as_str(),
        prepared.checked_form_id.as_str(),
        prepared.expanded_form_id.as_str(),
        prepared.plan.plan_id.as_str(),
        prepared.active_play.active_play_id.as_str(),
        identity::hex(&identities.host),
        identity::hex(&identities.boot),
        PHYSICAL_TRANSITIONS,
    );
    arch::early_write(sign.as_bytes());
    let snapshot = keyboard_text_observatory::completed_snapshot(
        record,
        identities,
        offer,
        &prepared,
        build_id,
        image_id,
        framebuffer,
    )
    .map_err(|_| "keyboard-text-observatory-refused")?;
    arch::early_write(keyboard_text_observatory::EXPORT_PREFIX.as_bytes());
    arch::early_write(snapshot.as_bytes());
    arch::early_write(b"\n");
    arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-text-completed\n");
    Ok(())
}

fn prepare(
    identities: &identity::BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<keyboard_text_plan::PreparedKeyboardTextPlay, &'static str> {
    keyboard_text_plan::prepare(identities, offer, build_id)
        .map_err(|_| "keyboard-text-plan-refused")
}

fn require(
    report: &keyboard_text_play::KeyboardTextPlayReport,
    expected: &[&[u8]],
) -> Result<(), &'static str> {
    if !report.completed
        || usize::from(report.presentation_count) != expected.len()
        || report
            .presentations
            .iter()
            .zip(expected)
            .any(|(actual, expected)| {
                actual
                    .map(|value| value.as_bytes() != *expected)
                    .unwrap_or(true)
            })
    {
        return Err("keyboard-text-semantic-result-invalid");
    }
    Ok(())
}
