//! Triggered observation of one exact current-value specialization.

#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, Kind, KindIdentity,
    KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection, PortTemporal,
    TerminalTransductionProfile, TERMINAL_INFO_ENCODED_LEN, TERMINAL_INFO_ID, UNIT_INFO_ID,
};

pub const CURRENT_SAMPLE_KIND: &str = "current/sample";
pub const CURRENT_SAMPLE_CONTRACT_REVISION: &str = "conduit.current/sample@1";

/// Specializes `current/sample` to exact checked value and trigger contracts.
///
/// Current replacements are observed and committed independently. Every
/// accepted trigger emits the last committed current value as one ordinary
/// flow item. The trigger owns terminal behavior; replacing the current value
/// never emits and never invents implicit Current-to-Value compatibility.
pub fn current_sample_semantic_contract(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != UNIT_INFO_ID {
        return Err("current/sample requires one finite canonical value envelope");
    }
    let value_port = |name: &str, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: (name == "value").then(|| kind_id(TERMINAL_INFO_ID)),
    };
    let trigger_port = PortDescriptor {
        port_id: port_id("trigger"),
        value_kind: trigger.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: false },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("current")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("trigger")),
            contract: trigger.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("value")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::InputAbnormal(port_id("trigger")),
            contract: CheckedValueContract::new(
                kind_id(TERMINAL_INFO_ID),
                TERMINAL_INFO_ENCODED_LEN as u32,
                vec![],
            )
            .expect("canonical terminal info has one exact finite envelope"),
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
        kind_id: kind_id(CURRENT_SAMPLE_KIND),
        kind_contract_revision: KindIdentity::from(CURRENT_SAMPLE_CONTRACT_REVISION),
        inputs: vec![
            value_port("current", PortDirection::Input, PortTemporal::Current),
            trigger_port,
        ],
        outputs: vec![value_port(
            "value",
            PortDirection::Output,
            PortTemporal::Flow { closes: false },
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
                .checked_add(trigger.maximum_bytes)
                .and_then(|bytes| bytes.checked_add(TERMINAL_INFO_ENCODED_LEN as u32))
                .ok_or("current/sample finite queue envelope overflows")?,
        },
    })
}

#[cfg(feature = "plot-catalog")]
pub fn install_current_sample_kind(
    value: &CheckedValueContract,
    trigger: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let contract = current_sample_semantic_contract(value, trigger).map_err(str::to_string)?;
    startup.insert(conduit_plot::KindSignature {
        kind: CURRENT_SAMPLE_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(CURRENT_SAMPLE_KIND, contract.checked_front())?;
    profile
        .insert_kind(contract)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_preserves_exact_types_and_separates_current_trigger_and_value_flow() {
        let text = CheckedValueContract::new(kind_id("value/text"), 4_096, vec![]).unwrap();
        let request = CheckedValueContract::new(kind_id("data/save-request@1"), 0, vec![]).unwrap();
        let contract = current_sample_semantic_contract(&text, &request).unwrap();
        assert_eq!(contract.inputs[0].temporal, PortTemporal::Current);
        assert_eq!(contract.inputs[1].value_kind, request.value_kind);
        assert_eq!(
            contract.inputs[1].temporal,
            PortTemporal::Flow { closes: false }
        );
        assert_eq!(
            contract.outputs[0].temporal,
            PortTemporal::Flow { closes: false }
        );
        assert_eq!(
            contract.inputs[0].value_kind,
            contract.outputs[0].value_kind
        );
        assert_eq!(
            contract.outputs[0].abnormal_kind.as_ref().unwrap().as_str(),
            TERMINAL_INFO_ID
        );
        assert!(matches!(
            contract
                .terminal_transductions()
                .next()
                .unwrap()
                .normal_close,
            NormalCloseTransduction::NotAccepted
        ));
    }
}
