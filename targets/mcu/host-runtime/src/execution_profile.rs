//! Const storage facts from complete checked Source tables, never authority.
//! Kernel construction independently validates actual topology and Back laws.
use conduit_kernel::scheduler::{
    AssignedAbnormalTransduction, AssignedTerminalTransduction, CanonicalValue,
};
/// Only finalization followed by propagation retains an abnormal phase value.
pub const fn phase_bytes<const N: usize, const P: usize>(
    table: &[[Option<AssignedTerminalTransduction>; P]; N],
) -> usize {
    let mut n = 0;
    while n < N {
        let mut p = 0;
        while p < P {
            if let Some(contract) = table[n][p] {
                if matches!(
                    contract.abnormal,
                    AssignedAbnormalTransduction::FinalizeThenPropagate(_)
                ) {
                    return CanonicalValue::MAXIMUM_BYTES;
                }
            }
            p += 1;
        }
        n += 1;
    }
    0
}
/// Unresolved abnormal publication truth needs stock only for routed abnormal tracks.
pub const fn unresolved_nodes(nodes: usize, abnormal_cords: usize) -> usize {
    if abnormal_cords == 0 {
        0
    } else {
        nodes
    }
}
/// Every external ingress retains full terminal truth, including local Fore.
/// Abnormal routing conservatively retains the default all-Cord stock.
pub const fn cord_terminals(cords: usize, abnormal_cords: usize, external_ingress: usize) -> usize {
    assert!(external_ingress <= cords);
    if abnormal_cords == 0 {
        external_ingress
    } else {
        cords
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_phase_and_actual_ingress_are_distinct_from_total_topology() {
        assert_eq!(phase_bytes(&[[None; 3]; 2]), 0);
        assert_eq!(unresolved_nodes(76, 0), 0);
        assert_eq!(unresolved_nodes(76, 1), 76);
        assert_eq!(cord_terminals(95, 0, 3), 3);
        assert_eq!(cord_terminals(95, 1, 3), 95);
        assert_eq!(cord_terminals(0, 0, 0), 0);
    }
    #[test]
    fn finalization_anywhere_in_complete_table_retains_full_abnormal_value() {
        use conduit_kernel::scheduler::{
            AssignedCancellationTransduction, AssignedFiniteTerminalEmission,
            AssignedNormalCloseTransduction,
        };
        let mut table = [[None; 3]; 2];
        let mut contract = AssignedTerminalTransduction {
            input: conduit_kernel::PortId(0),
            output: conduit_kernel::PortId(1),
            normal_close: AssignedNormalCloseTransduction::PropagateAfterDrain,
            abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        };
        table[1][2] = Some(contract);
        assert_eq!(phase_bytes(&table), 0);
        contract.abnormal =
            AssignedAbnormalTransduction::FinalizeThenPropagate(AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: 1,
            });
        table[1][2] = Some(contract);
        assert_eq!(phase_bytes(&table), CanonicalValue::MAXIMUM_BYTES);
    }
    #[test]
    #[should_panic]
    fn invalid_ingress_fact_refuses_const_profile() {
        cord_terminals(2, 0, 3);
    }
}
