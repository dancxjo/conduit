//! Diagnostic capacity fixture; ordinary editor evidence belongs to the Body journey.
use super::{gates, refuse};
use crate::offer::HostOffer;
use conduit_core::Plan;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

pub(super) fn run(plan: &Plan, offer: &HostOffer<'_>) {
    let mut domain = gates::fixture(plan, offer);
    domain
        .reset_editor(1)
        .unwrap_or_else(|_| refuse("editor-fixture-initialize"));
    for (usage, source, text, refused) in [
        (0x04, "a", "a", false),
        (0x05, "b", "", true),
        (0x2a, "\u{8}", "", false),
        (0x06, "c", "c", false),
    ] {
        let event = KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE)
            .unwrap_or_else(|_| refuse("editor-fixture-event"));
        let result = domain
            .keymap_chain(&event.encode())
            .unwrap_or_else(|_| refuse("editor-fixture-chain"))
            .unwrap_or_else(|| refuse("editor-fixture-source-absent"));
        if result.source() != source.as_bytes()
            || result.transformed() != text.as_bytes()
            || result.edit_refused() != refused
        {
            refuse("editor-fixture-capacity-or-state");
        }
    }
    crate::arch::early_write(
        b"CONDUIT_DOMAIN_EDITOR_FIXTURE bounded-capacity retained-state edit-refusal\n",
    );
}
