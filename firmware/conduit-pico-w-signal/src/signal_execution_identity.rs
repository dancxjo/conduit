//! Plan-specific identity carried through the shared Signal kernel shape.

use crate::receipts::{PresentationReceiptIdentity, TerminalIdentity};

#[derive(Clone, Copy)]
pub struct SignalExecutionIdentity {
    pub firmware_build_id: &'static str,
    pub source_document_id: &'static str,
    pub checked_form_id: &'static str,
    pub expanded_form_id: &'static str,
    pub plan_id: &'static str,
    pub fragment_id: &'static str,
    pub host_id: &'static str,
    pub boot_id: &'static str,
    pub active_play_id: &'static str,
    pub terminal_clue_id: &'static str,
    pub(crate) presentation_ids: &'static [&'static str],
    pub(crate) presentation_clue_ids: &'static [&'static str],
}

impl SignalExecutionIdentity {
    pub fn plan_a() -> Self {
        Self {
            firmware_build_id: crate::signal_image::FIRMWARE_BUILD_ID,
            source_document_id: crate::signal_image::SOURCE_DOCUMENT_ID,
            checked_form_id: crate::signal_image::CHECKED_FORM_ID,
            expanded_form_id: crate::signal_image::EXPANDED_FORM_ID,
            plan_id: crate::signal_image::PLAN_ID,
            fragment_id: crate::signal_image::FRAGMENT_ID,
            host_id: crate::signal_image::HOST_ID,
            boot_id: crate::signal_image::BOOT_ID,
            active_play_id: crate::signal_image::ACTIVE_PLAY_ID,
            terminal_clue_id: crate::signal_image::TERMINAL_CLUE_ID,
            presentation_ids: crate::signal_image::presentation_ids(),
            presentation_clue_ids: crate::signal_image::presentation_clue_ids(),
        }
    }

    #[cfg(feature = "wifi-bootstrap")]
    pub fn plan_b() -> Self {
        crate::plan_b_signal_image::execution_identity()
    }

    pub fn presentation(self, sequence: usize) -> Option<PresentationReceiptIdentity> {
        Some(PresentationReceiptIdentity {
            firmware_build_id: self.firmware_build_id,
            source_document_id: self.source_document_id,
            checked_form_id: self.checked_form_id,
            expanded_form_id: self.expanded_form_id,
            plan_id: self.plan_id,
            fragment_id: self.fragment_id,
            host_id: self.host_id,
            boot_id: self.boot_id,
            active_play_id: self.active_play_id,
            presentation_id: self.presentation_ids.get(sequence)?,
            clue_id: self.presentation_clue_ids.get(sequence)?,
        })
    }

    pub fn terminal(self) -> TerminalIdentity {
        TerminalIdentity {
            firmware_build_id: self.firmware_build_id,
            source_document_id: self.source_document_id,
            checked_form_id: self.checked_form_id,
            expanded_form_id: self.expanded_form_id,
            plan_id: self.plan_id,
            fragment_id: self.fragment_id,
            host_id: self.host_id,
            boot_id: self.boot_id,
            active_play_id: self.active_play_id,
            clue_id: self.terminal_clue_id,
        }
    }
}
