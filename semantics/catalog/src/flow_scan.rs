//! Exact bounded retained progression through a reviewed two-input combine Form.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FlowScanAbnormalDisposition,
    FlowScanCancellationDisposition, FlowScanCloseDisposition, FlowScanEmptyDisposition,
    FlowScanInvocation, FlowScanProgression, FlowScanSemanticLaw, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const FLOW_SCAN_KIND: &str = "flow/scan";
pub const FLOW_SCAN_CONTRACT_REVISION: &str = "conduit.flow/scan@1";
// These are the coordinator's exact internal Fore names. The enclosing Form
// may export differently named public Flow ports.
pub const FLOW_SCAN_INPUT_PORT: &str = "item";
pub const FLOW_SCAN_OUTPUT_PORT: &str = "combined";
pub const FLOW_SCAN_COMBINE_ACCUMULATOR_PORT: &str = "accumulator";
pub const FLOW_SCAN_COMBINE_ITEM_PORT: &str = "item";
pub const FLOW_SCAN_COMBINE_OUTPUT_PORT: &str = "combined";

pub fn flow_scan_semantic_contract(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: &[u8],
    abnormal: Option<&CheckedValueContract>,
) -> Result<Kind, &'static str> {
    require_finite(item, "item")?;
    require_finite(accumulator, "accumulator")?;
    if accumulator.validate(initial_accumulator).is_err() {
        return Err("flow/scan initial value is not canonical for its exact accumulator contract");
    }
    if let Some(abnormal) = abnormal {
        require_finite(abnormal, "abnormal terminal")?;
    }
    let abnormal_kind = abnormal.map(|value| value.value_kind.clone());
    let input = PortDescriptor {
        port_id: port_id(FLOW_SCAN_INPUT_PORT),
        value_kind: item.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: abnormal_kind.clone(),
    };
    let output = PortDescriptor {
        port_id: port_id(FLOW_SCAN_OUTPUT_PORT),
        value_kind: accumulator.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind,
    };
    let mut contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(input.port_id.clone()),
            contract: item.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(output.port_id.clone()),
            contract: accumulator.clone(),
        },
    ];
    if let Some(abnormal) = abnormal {
        contracts.extend([
            FrontValueContract {
                location: FrontValueLocation::InputAbnormal(input.port_id.clone()),
                contract: abnormal.clone(),
            },
            FrontValueContract {
                location: FrontValueLocation::OutputAbnormal(output.port_id.clone()),
                contract: abnormal.clone(),
            },
        ]);
    }
    let terminal_bytes = abnormal.map_or(0, |value| value.maximum_bytes);
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_SCAN_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_SCAN_CONTRACT_REVISION),
        inputs: vec![input],
        outputs: vec![output],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(contracts),
            KindSemanticLaw::FlowScan(FlowScanSemanticLaw {
                input_port_id: port_id(FLOW_SCAN_INPUT_PORT),
                output_port_id: port_id(FLOW_SCAN_OUTPUT_PORT),
                item: item.clone(),
                accumulator: accumulator.clone(),
                initial_accumulator: initial_accumulator.to_vec(),
                combine_accumulator_port_id: port_id(FLOW_SCAN_COMBINE_ACCUMULATOR_PORT),
                combine_item_port_id: port_id(FLOW_SCAN_COMBINE_ITEM_PORT),
                combine_output_port_id: port_id(FLOW_SCAN_COMBINE_OUTPUT_PORT),
                maximum_active: 1,
                maximum_queued: 1,
                invocation: FlowScanInvocation::OncePerAcceptedInput,
                progression: FlowScanProgression::EmitCombinedAccumulatorExactlyOnceInInputOrder,
                empty: FlowScanEmptyDisposition::EmitNothing,
                close: FlowScanCloseDisposition::DrainThenCloseWithoutExtraEmission,
                abnormal: FlowScanAbnormalDisposition::DiscardAccumulatorAndPropagateExact,
                cancellation: FlowScanCancellationDisposition::DiscardAccumulatorWithoutEmission,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            // retained accumulator, active item, queued item, and pending output
            max_queue_items: 4,
            max_queue_bytes: accumulator
                .maximum_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(item.maximum_bytes.checked_mul(2)?))
                .and_then(|bytes| bytes.checked_add(terminal_bytes))
                .ok_or("flow/scan finite storage envelope overflows")?,
        },
    })
}

fn require_finite(value: &CheckedValueContract, role: &'static str) -> Result<(), &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != conduit_core::UNIT_INFO_ID {
        return Err(match role {
            "item" => "flow/scan requires a finite canonical item envelope",
            "accumulator" => "flow/scan requires a finite canonical accumulator envelope",
            _ => "flow/scan requires a finite canonical abnormal terminal envelope",
        });
    }
    Ok(())
}

#[cfg(feature = "form-catalog")]
pub fn install_flow_scan_kind(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial_accumulator: &[u8],
    abnormal: Option<&CheckedValueContract>,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: FLOW_SCAN_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(
            flow_scan_semantic_contract(item, accumulator, initial_accumulator, abnormal)
                .map_err(str::to_string)?,
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(kind: &str, bytes: u32) -> CheckedValueContract {
        CheckedValueContract::new(kind_id(kind), bytes, vec![]).unwrap()
    }

    #[test]
    fn progression_emits_once_per_item_and_never_emits_initial_or_close_value() {
        let initial = 7_u64.to_le_bytes();
        let kind = flow_scan_semantic_contract(
            &value("value/u32", 4),
            &value("value/u64", 8),
            &initial,
            Some(&value("terminal/scan", 2)),
        )
        .unwrap();
        let KindSemanticLaw::FlowScan(law) = &kind.semantic_laws[1] else {
            panic!()
        };
        assert_eq!(law.initial_accumulator, initial);
        assert_eq!(law.invocation, FlowScanInvocation::OncePerAcceptedInput);
        assert_eq!(
            law.progression,
            FlowScanProgression::EmitCombinedAccumulatorExactlyOnceInInputOrder
        );
        assert_eq!(law.empty, FlowScanEmptyDisposition::EmitNothing);
        assert_eq!(
            law.close,
            FlowScanCloseDisposition::DrainThenCloseWithoutExtraEmission
        );
        assert_eq!(
            kind.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(kind.limits.max_queue_items, 4);
        assert_eq!(kind.limits.max_queue_bytes, 26);
        kind.validate().unwrap();
    }

    #[test]
    fn terminal_dispositions_discard_state_without_emission() {
        let kind = flow_scan_semantic_contract(
            &value("value/u32", 4),
            &value("value/u64", 8),
            &0_u64.to_le_bytes(),
            None,
        )
        .unwrap();
        let KindSemanticLaw::FlowScan(law) = &kind.semantic_laws[1] else {
            panic!()
        };
        assert_eq!(
            law.abnormal,
            FlowScanAbnormalDisposition::DiscardAccumulatorAndPropagateExact
        );
        assert_eq!(
            law.cancellation,
            FlowScanCancellationDisposition::DiscardAccumulatorWithoutEmission
        );
        assert_eq!(law.maximum_active, 1);
        assert_eq!(law.maximum_queued, 1);
    }

    #[test]
    fn refuses_inexact_initial_and_unbounded_storage() {
        let item = value("value/u32", 4);
        let accumulator = value("value/u64", 8);
        assert!(flow_scan_semantic_contract(&item, &accumulator, &[0; 7], None).is_err());
        assert!(flow_scan_semantic_contract(
            &value("value/unbounded", 0),
            &accumulator,
            &[0; 8],
            None
        )
        .is_err());
    }
}
