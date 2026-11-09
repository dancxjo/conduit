//! Fixed typed append-status Fore custody and unsigned counters.
use super::{Refusal, State};
use serde::Serialize;
/// A single fixed local Fore owner, funded before Start. This is not a Line or
/// a remote recording acknowledgment; it consumes only the exact selected ABI.
pub struct PreparedAppendStatus {
    boundary: Option<conduit_core::AssignedForeRequirement>,
    bytes: [u8; conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES as usize],
    length: usize,
    pending: Option<(
        u64,
        conduit_resource_snapshot::append_result::DecodedAppendResult,
    )>,
}
impl PreparedAppendStatus {
    pub fn prepare(
        actual: &[conduit_core::AssignedForeRequirement],
        expected: Option<conduit_core::AssignedForeRequirement>,
    ) -> Result<Self, Refusal> {
        match (actual, expected) {
            ([], None) => (),
            ([actual], Some(expected))
                if actual.payload() == expected.payload()
                    && actual.direction() == conduit_core::AssignedForeDirection::Output => {}
            _ => return Err(Refusal::AppendStatus),
        }
        Ok(Self {
            boundary: expected,
            bytes: [0; conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES as usize],
            length: 0,
            pending: None,
        })
    }
    pub fn boundary(&self) -> Option<conduit_core::AssignedForeRequirement> {
        self.boundary
    }
    pub fn stage(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), Refusal> {
        if let Some((prior, _)) = self.pending {
            return if prior == sequence && bytes == &self.bytes[..self.length] {
                Ok(())
            } else {
                Err(Refusal::AppendStatus)
            };
        }
        if bytes.len() > self.bytes.len() {
            return Err(Refusal::AppendStatus);
        }
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        let outcome = conduit_resource_snapshot::append_result::decode(&self.bytes[..bytes.len()])
            .map_err(|_| Refusal::AppendStatus)?;
        self.length = bytes.len();
        self.pending = Some((sequence, outcome));
        Ok(())
    }
    pub fn delivered<const F: usize>(
        &mut self,
        state: &State<F>,
        sequence: u64,
    ) -> Result<(), Refusal> {
        let (prior, outcome) = self.pending.ok_or(Refusal::AppendStatus)?;
        if prior != sequence {
            return Err(Refusal::AppendStatus);
        }
        let mut next = state.append.get();
        use conduit_resource_snapshot::append_result::DecodedAppendResult::*;
        let (counter, label) = match outcome {
            Recorded(index) => {
                next.last_recorded_index = Some(index);
                (&mut next.recorded, "recorded")
            }
            Capacity => (&mut next.capacity, "capacity"),
            Uncertain => (&mut next.uncertain, "uncertain"),
            RecoveryRequired => (&mut next.recovery_required, "recovery-required"),
        };
        *counter = counter.checked_add(1).ok_or(Refusal::Capacity)?;
        next.last = Some(label);
        state.append.set(next);
        self.pending = None;
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct AppendInspection {
    recorded: u64,
    capacity: u64,
    uncertain: u64,
    recovery_required: u64,
    last_recorded_index: Option<u32>,
    last: Option<&'static str>,
}

#[cfg(test)]
mod append_status_tests {
    use super::*;
    use conduit_core::{
        AssignedForeDirection as D, AssignedForeRequirement as F, AssignedIdentity as I,
    };
    #[test]
    fn exact_selected_output_abi_is_required_before_start() {
        let output = F::new(1, 2, D::Output, I::from_text("append_status"), [9; 32]);
        assert!(PreparedAppendStatus::prepare(&[], None).is_ok());
        assert!(PreparedAppendStatus::prepare(&[output], Some(output)).is_ok());
        assert!(PreparedAppendStatus::prepare(&[output], None).is_err());
        assert!(PreparedAppendStatus::prepare(&[], Some(output)).is_err());
        assert!(PreparedAppendStatus::prepare(&[output, output], Some(output)).is_err());
        let foreign = F::new(1, 2, D::Output, I::from_text("append_status"), [8; 32]);
        assert!(PreparedAppendStatus::prepare(&[foreign], Some(output)).is_err());
        let mut ready = PreparedAppendStatus::prepare(&[output], Some(output)).unwrap();
        assert!(ready.stage(0, b"invalid").is_err());
        assert!(ready.pending.is_none());
    }
}
