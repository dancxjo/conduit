//! Bounded preparation diagnostics; preserve every original refusal.
use super::ProtocolCallRefusal;
use core::fmt::{Debug, Write};

pub(crate) fn observe<E: Debug>(stage: &str, error: E) -> E {
    if crate::allocation::BOOT_ARENA.capacity() != 0 {
        let mut line = crate::sign_format::FixedText::new();
        if writeln!(
            line,
            "CONDUIT_PROTOCOL_PLAN_REFUSAL stage={stage} reason={error:?}"
        )
        .is_ok()
        {
            crate::arch::early_write(line.as_bytes());
        }
    }
    error
}

pub(crate) fn invalid<E: Debug>(stage: &str, error: E) -> ProtocolCallRefusal {
    observe(stage, error);
    ProtocolCallRefusal::InvalidPlan
}

pub(crate) fn lowering(
    fragment: &conduit_core::PlanFragment,
    error: conduit_plan_lowering::lowering::LoweringError,
) -> ProtocolCallRefusal {
    if let conduit_plan_lowering::lowering::LoweringError::ConnectionContractMismatch(id) = &error
        && let Some(connection) = fragment
            .connections
            .iter()
            .find(|cord| &cord.connection_id == id)
    {
        observe(
            "connection",
            (
                connection.track,
                connection.temporal,
                &connection.value_kind,
            ),
        );
        for (stage, placement_id, port_id, input) in [
            (
                "connection/source",
                &connection.source_placement_id,
                &connection.source_port_id,
                false,
            ),
            (
                "connection/sink",
                &connection.sink_placement_id,
                &connection.sink_port_id,
                true,
            ),
        ] {
            if let Some(gear) = fragment
                .placements
                .iter()
                .find(|gear| &gear.placement_id == placement_id)
            {
                let ports = if input { &gear.inputs } else { &gear.outputs };
                observe(
                    stage,
                    (
                        &gear.implementation_id,
                        &gear.gear_id,
                        ports.iter().find(|port| &port.port_id == port_id),
                    ),
                );
                for contract in gear.checked_port_front().value_contracts() {
                    observe("connection/value-contract", contract);
                }
            }
        }
    }
    invalid("pure/lowering", error)
}
