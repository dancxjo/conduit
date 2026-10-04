//! Exact finite two-input merge, independent of any carrier or device protocol.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const FLOW_MERGE_FINITE_KIND: &str = "flow/merge/finite";
pub fn flow_merge_finite_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    // Retain the same checked schema eligibility as finite typed state.
    super::seeded_state_semantic_contract(value, schema)?;
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let mut laws = vec![KindSemanticLaw::ValueContracts(
        [
            ("left", FrontValueLocation::Input(port_id("left"))),
            ("right", FrontValueLocation::Input(port_id("right"))),
            ("merged", FrontValueLocation::Output(port_id("merged"))),
        ]
        .into_iter()
        .map(|(_, location)| FrontValueContract {
            location,
            contract: value.clone(),
        })
        .collect(),
    )];
    for input in ["left", "right"] {
        laws.push(KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: port_id(input),
                output_port_id: port_id("merged"),
                normal_close: NormalCloseTransduction::PropagateWhenAllClose,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                cancellation: CancellationTransduction::NotCancellable,
            },
        ));
    }
    Ok(Kind {
        kind_id: kind_id(FLOW_MERGE_FINITE_KIND),
        kind_contract_revision: KindIdentity::from("conduit.flow/merge-finite@2"),
        startup_parameters: Vec::new(),
        shorthand: None,
        inputs: vec![
            port("left", PortDirection::Input),
            port("right", PortDirection::Input),
        ],
        outputs: vec![port("merged", PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: laws,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(2)
                .ok_or("finite merge envelope overflows")?
                .max(1),
        },
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_flow_merge_finite_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind =
        flow_merge_finite_semantic_contract(value, schema).map_err(alloc::string::String::from)?;
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_MERGE_FINITE_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert_fore(FLOW_MERGE_FINITE_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_merge_ports_retain_equal_bounds_and_independent_terminal_contracts() {
        let schema = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let value = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
        let kind = flow_merge_finite_semantic_contract(&value, &schema).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.inputs.len(), 2);
        assert_eq!(kind.outputs.len(), 1);
        assert_eq!(kind.terminal_transductions().count(), 2);
        assert!(kind
            .value_contracts()
            .iter()
            .all(|entry| entry.contract == value));
        assert!(kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .all(|port| port.temporal == PortTemporal::Flow { closes: true }
                && port.abnormal_kind.is_none()));
        let wrong = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        assert!(flow_merge_finite_semantic_contract(&value, &wrong).is_err());
    }
}
