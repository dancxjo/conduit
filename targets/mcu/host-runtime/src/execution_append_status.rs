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
    /// Own only the independently selected output within a complete Fore
    /// inventory. This does not own, consume, or deliver any other boundary;
    /// the complete inventory still requires ordinary kernel/profile checks.
    /// `None` explicitly selects no observer, regardless of other API ports.
    pub fn prepare_selected(
        actual: &[conduit_core::AssignedForeRequirement],
        expected: Option<conduit_core::AssignedForeRequirement>,
    ) -> Result<Self, Refusal> {
        let Some(expected) = expected else {
            return Self::prepare(&[], None);
        };
        if expected.direction() != conduit_core::AssignedForeDirection::Output {
            return Err(Refusal::AppendStatus);
        }
        // Reject aliases of either numeric identity, including a second row
        // with the right boundary but the wrong ABI/direction.
        let mut selected = actual.iter().filter(|port| {
            port.cord() == expected.cord() || port.endpoint() == expected.endpoint()
        });
        let port = selected.next().ok_or(Refusal::AppendStatus)?;
        if selected.next().is_some() || port.payload() != expected.payload() {
            return Err(Refusal::AppendStatus);
        }
        Self::prepare(core::slice::from_ref(port), Some(expected))
    }
    pub fn boundary(&self) -> Option<conduit_core::AssignedForeRequirement> {
        self.boundary
    }
    pub fn stage(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), Refusal> {
        if self.boundary.is_none() {
            return Err(Refusal::AppendStatus);
        }
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
    #[test]
    fn selected_observer_owns_only_status_in_complete_three_port_inventory() {
        let status = F::new(0, 2, D::Output, I::from_text("append_status"), [9; 32]);
        let read_index = F::new(1, 3, D::Input, I::from_text("read_index"), [7; 32]);
        let read_result = F::new(2, 4, D::Output, I::from_text("read_result"), [8; 32]);
        let inventory = [read_index, status, read_result];
        let before = inventory.map(|port| *port.payload());
        let mut ready = PreparedAppendStatus::prepare_selected(&inventory, Some(status)).unwrap();
        assert_eq!(ready.boundary().unwrap().payload(), status.payload());
        for other in [read_index, read_result] {
            assert_ne!(ready.boundary().unwrap().cord(), other.cord());
            assert_ne!(ready.boundary().unwrap().endpoint(), other.endpoint());
        }
        assert!(ready.pending.is_none());
        assert_eq!(inventory.map(|port| *port.payload()), before);
        let unit = conduit_core::StructuredInfoValue::leaf(
            conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/empty")).unwrap(),
            std::vec::Vec::new(),
        )
        .unwrap();
        let capacity = conduit_core::StructuredInfoValue::variant(
            conduit_resource_snapshot::append_result::result_type().unwrap(),
            "capacity",
            unit,
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert!(ready.stage(0, &capacity).is_ok());
        assert!(ready.pending.is_some());
        let mut absent = PreparedAppendStatus::prepare_selected(&inventory, None).unwrap();
        assert!(absent.boundary().is_none());
        assert!(absent.stage(0, &capacity).is_err());
        assert!(absent.pending.is_none());
        // The strict old API continues to reject unrelated ports.
        assert!(PreparedAppendStatus::prepare(&inventory, Some(status)).is_err());
    }
    #[test]
    fn selected_observer_rejects_missing_aliases_and_wrong_contract() {
        let status = F::new(0, 2, D::Output, I::from_text("append_status"), [9; 32]);
        let read = F::new(1, 3, D::Input, I::from_text("read_index"), [7; 32]);
        assert!(PreparedAppendStatus::prepare_selected(&[read], Some(status)).is_err());
        for wrong in [
            status,
            F::new(0, 2, D::Input, I::from_text("append_status"), [9; 32]),
            F::new(0, 2, D::Output, I::from_text("append_status"), [8; 32]),
            F::new(0, 2, D::Output, I::from_text("read_result"), [9; 32]),
            F::new(0, 5, D::Output, I::from_text("append_status"), [9; 32]),
            F::new(5, 2, D::Output, I::from_text("append_status"), [9; 32]),
        ] {
            assert!(
                PreparedAppendStatus::prepare_selected(&[read, status, wrong], Some(status))
                    .is_err()
            );
            if wrong.payload() != status.payload() {
                assert!(
                    PreparedAppendStatus::prepare_selected(&[read, wrong], Some(status)).is_err()
                );
            }
        }
        let input = F::new(0, 2, D::Input, I::from_text("append_status"), [9; 32]);
        assert!(PreparedAppendStatus::prepare_selected(&[input], Some(input)).is_err());
    }
}
