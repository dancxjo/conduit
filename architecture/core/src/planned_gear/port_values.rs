//! Borrowed resolution of the same checked directional value contract.
use crate::{CheckedValueContract, FrontValueLocation, KindId, PlannedGear, PortDirection, PortId};
impl PlannedGear {
    fn explicit_port_contract(
        &self,
        direction: PortDirection,
        id: &PortId,
        abnormal: bool,
    ) -> Option<&CheckedValueContract> {
        self.semantic_contract
            .value_contracts()
            .iter()
            .rev()
            .find(|entry| match (&entry.location, direction, abnormal) {
                (FrontValueLocation::Input(port), PortDirection::Input, false)
                | (FrontValueLocation::Output(port), PortDirection::Output, false)
                | (FrontValueLocation::InputAbnormal(port), PortDirection::Input, true)
                | (FrontValueLocation::OutputAbnormal(port), PortDirection::Output, true) => {
                    port == id
                }
                _ => false,
            })
            .map(|entry| &entry.contract)
    }
    fn port_kind(&self, direction: PortDirection, id: &PortId, abnormal: bool) -> Option<&KindId> {
        let ports = match direction {
            PortDirection::Input => &self.inputs,
            PortDirection::Output => &self.outputs,
        };
        ports
            .iter()
            .filter(|port| &port.port_id == id)
            .filter_map(|port| {
                if abnormal {
                    port.abnormal_kind.as_ref()
                } else {
                    Some(&port.value_kind)
                }
            })
            .filter(|kind| crate::front::default_maximum_bytes(kind).is_some())
            .min()
    }
    /// The explicit last location override or canonical default, without cloning a Fore.
    pub fn port_value_maximum_bytes(
        &self,
        direction: PortDirection,
        id: &PortId,
        abnormal: bool,
    ) -> Option<u64> {
        if let Some(contract) = self.explicit_port_contract(direction, id, abnormal) {
            return Some(u64::from(contract.maximum_bytes));
        }
        crate::front::default_maximum_bytes(self.port_kind(direction, id, abnormal)?)
    }
    pub(crate) fn port_value_contract_matches(
        &self,
        direction: PortDirection,
        id: &PortId,
        abnormal: bool,
        supplied: Option<&CheckedValueContract>,
    ) -> bool {
        if let Some(contract) = self.explicit_port_contract(direction, id, abnormal) {
            return Some(contract) == supplied;
        }
        let expected = self.port_kind(direction, id, abnormal).and_then(|kind| {
            crate::front::default_maximum_bytes(kind).map(|maximum| (kind, maximum))
        });
        match (expected, supplied) {
            (None, None) => true,
            (Some((kind, maximum)), Some(contract)) => {
                &contract.value_kind == kind
                    && u64::from(contract.maximum_bytes) == maximum
                    && contract.constraints.is_empty()
            }
            _ => false,
        }
    }
}
