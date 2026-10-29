//! Finite producer staging; queue acceptance is not document mutation.
use super::*;

impl BodyLiveForeQueue {
    pub fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        if self.control.stop_requested() {
            return Err("live Fore Play is cancelling".into());
        }
        if canonical.len() > self.port.byte_capacity as usize {
            return Err("live Fore value exceeds its sealed byte bound".into());
        }
        if self.port.value_kind.as_str() == conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND {
            conduit_semantic_catalog::TextEdit::decode(canonical)
                .map_err(|error| format!("document edit representation: {error:?}"))?;
        }
        if let Some(contract) = &self.port.value_contract {
            contract
                .validate(canonical)
                .map_err(|error| format!("live Fore value contract: {error:?}"))?;
        } else {
            conduit_core::validate_primitive_info(self.port.value_kind.as_str(), canonical)
                .map_err(|error| format!("live Fore value kind: {error:?}"))?;
        }
        if self.initial.is_some()
            && self.port.value_kind.as_str() == conduit_todo_plot::TODO_COMMAND_INFO_ID
        {
            conduit_todo_plot::TodoCommand::decode_info(canonical)
                .map_err(|error| format!("waiting checkpoint command: {error:?}"))?;
        }
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if self.initial.is_some() && !state.play_started {
            return Err("waiting checkpoint command requires a started Body Play".into());
        }
        if state.close_requested || state.kernel_closed || state.play_terminal {
            return Err("live Fore input is closed".into());
        }
        if state.submitted == self.maximum_items {
            return Err("live Fore selected item bound is exhausted".into());
        }
        let Some(index) = state.free.pop() else {
            return Ok(BodyLiveForeAdmission::Full);
        };
        let sequence = u64::from(state.submitted);
        state.slots[index].clear();
        state.slots[index].extend_from_slice(canonical);
        state.queued.push_back(index);
        state.submitted += 1;
        drop(state);
        self.control.signal_activity();
        Ok(BodyLiveForeAdmission::Accepted { sequence })
    }
}
