//! One-shot observation of one exact current value specialization.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, Kind, KindIdentity,
    KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection, PortTemporal,
    TerminalTransductionProfile, TERMINAL_INFO_ENCODED_LEN, TERMINAL_INFO_ID, UNIT_INFO_ID,
};

pub const STATE_SNAPSHOT_KIND: &str = "state/snapshot";
pub const STATE_SNAPSHOT_CONTRACT_REVISION: &str = "conduit.state/snapshot@1";

/// Specializes `state/snapshot` to one already-checked finite value contract.
///
/// Current replacements are observed and committed independently. One Unit
/// trigger emits the last committed current value as one ordinary value. The
/// trigger owns terminal behavior; closing the current input does not consume
/// or erase the last observed generation.
pub fn state_snapshot_semantic_contract(
    value: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != UNIT_INFO_ID {
        return Err("state/snapshot requires one finite canonical value envelope");
    }
    let value_port = |name: &str, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: (name == "value").then(|| kind_id(TERMINAL_INFO_ID)),
    };
    let trigger = PortDescriptor {
        port_id: port_id("trigger"),
        value_kind: kind_id(UNIT_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let unit = CheckedValueContract::new(kind_id(UNIT_INFO_ID), 0, vec![])
        .expect("Unit has one exact zero-byte canonical representation");
    let value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("current")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("trigger")),
            contract: unit,
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("value")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::OutputAbnormal(port_id("value")),
            contract: CheckedValueContract::new(
                kind_id(TERMINAL_INFO_ID),
                TERMINAL_INFO_ENCODED_LEN as u32,
                vec![],
            )
            .expect("canonical terminal info has one exact finite envelope"),
        },
    ];
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(STATE_SNAPSHOT_KIND),
        kind_contract_revision: KindIdentity::from(STATE_SNAPSHOT_CONTRACT_REVISION),
        inputs: vec![
            value_port("current", PortDirection::Input, PortTemporal::Current),
            trigger,
        ],
        outputs: vec![value_port(
            "value",
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(value_contracts),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: port_id("trigger"),
                output_port_id: port_id("value"),
                normal_close: NormalCloseTransduction::NotAccepted,
                abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_add(TERMINAL_INFO_ENCODED_LEN as u32)
                .ok_or("state/snapshot finite queue envelope overflows")?,
        },
    })
}

#[cfg(feature = "form-catalog")]
pub fn install_state_snapshot_kind(
    value: &CheckedValueContract,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: STATE_SNAPSHOT_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(state_snapshot_semantic_contract(value).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_preserves_exact_type_and_separates_current_trigger_and_value() {
        let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
        let contract = state_snapshot_semantic_contract(&text).unwrap();
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Current);
        assert_eq!(contract.inputs[1].value_kind.as_str(), UNIT_INFO_ID);
        assert_eq!(contract.inputs[1].temporal, PortTemporal::Value);
        assert_eq!(contract.outputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
        assert_eq!(
            contract.outputs[0].abnormal_kind.as_ref().unwrap().as_str(),
            TERMINAL_INFO_ID
        );
        assert!(matches!(
            contract.terminal_transduction().unwrap().normal_close,
            NormalCloseTransduction::NotAccepted
        ));
    }
}
