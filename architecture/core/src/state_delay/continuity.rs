//! Immutable provenance for retained State; identity is not transfer authority.
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::{PlannedStateBoundary, StateContinuation, StateId};
use crate::{bind_active_play, ActivePlayIdentity, FormIdentity, KindId};

/// Describes the exact value a replacement must obtain from a retired owner.
/// This record does not prove retirement, authorize migration, convey grants,
/// or allow a Host to reconstruct a fresh cell and call it continuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedStateProvenance {
    pub source_form: FormIdentity,
    pub source_play: ActivePlayIdentity,
    pub source_state: StateId,
    pub value_kind: KindId,
    pub generation: u64,
    pub current_value: Vec<u8>,
}

impl RetainedStateProvenance {
    pub(super) fn valid_for(&self, state: &PlannedStateBoundary) -> bool {
        let play = &self.source_play;
        !self.source_form.source_document_id.as_str().is_empty()
            && !self.source_form.checked_form_id.as_str().is_empty()
            && !self.source_form.expanded_form_id.as_str().is_empty()
            && !play.plan_id.as_str().is_empty()
            && !play.host_id.as_str().is_empty()
            && !play.boot_id.as_str().is_empty()
            && *play
                == bind_active_play(
                    &play.plan_id,
                    &play.host_id,
                    &play.boot_id,
                    play.play_sequence,
                )
            && self.source_state == state.state_id
            && self.value_kind == state.value_kind
            && self.current_value.len() <= state.maximum_value_bytes as usize
            && match state.continuation {
                StateContinuation::MaximumTransitions(maximum) => self.generation <= maximum,
                StateContinuation::ExternallyBounded => true,
            }
    }

    pub(super) fn push_canonical(&self, bytes: &mut Vec<u8>) {
        crate::push_string(bytes, "conduit/retained-state@1");
        for identity in [
            self.source_form.source_document_id.as_str(),
            self.source_form.checked_form_id.as_str(),
            self.source_form.expanded_form_id.as_str(),
            self.source_play.active_play_id.as_str(),
            self.source_play.plan_id.as_str(),
            self.source_play.host_id.as_str(),
            self.source_play.boot_id.as_str(),
            self.source_state.as_str(),
            self.value_kind.as_str(),
        ] {
            crate::push_string(bytes, identity);
        }
        crate::push_u64(bytes, self.source_play.play_sequence);
        crate::push_u64(bytes, self.generation);
        crate::push_u64(bytes, self.current_value.len() as u64);
        bytes.extend_from_slice(&self.current_value);
    }
}
