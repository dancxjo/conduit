//! Exact bounded pairing of the next value from each of two independent Flows.

#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FiniteTerminalEmission, FrontValueContract, FrontValueLocation, Kind,
    KindIdentity, KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection,
    PortTemporal, PreparedTuplePairEncoder, PreparedTypedTuplePairEncoder, StructuredInfoType,
    StructuredInfoTypeShape, TerminalTransductionProfile, EMPTY_INFO_ID, TERMINAL_INFO_ENCODED_LEN,
    TERMINAL_INFO_ID,
};

pub const FLOW_ZIP_KIND: &str = "flow/zip";
pub const FLOW_ZIP_CONTRACT_REVISION: &str = "conduit.flow/zip@1";

/// Specializes `flow/zip` over two exact checked value contracts.
///
/// At most one value from each side is pending. A pair is emitted only when
/// both are present. Normal close may finish the one already-formable pair and
/// then closes; any unmatched value is discarded. Abnormal truth never runs
/// the normal-close flush and propagates after queued input has drained.
pub fn flow_zip_semantic_contract(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    if (left.maximum_bytes == 0 && left.value_kind.as_str() != EMPTY_INFO_ID)
        || (right.maximum_bytes == 0 && right.value_kind.as_str() != EMPTY_INFO_ID)
    {
        return Err("flow/zip requires finite canonical input envelopes");
    }
    let encoder = PreparedTuplePairEncoder::new(
        left.value_kind.clone(),
        left.maximum_bytes,
        right.value_kind.clone(),
        right.maximum_bytes,
    )
    .map_err(|_| "flow/zip pair envelope exceeds structured Info bounds")?;
    let paired = CheckedValueContract::new(
        encoder
            .value_type()
            .map_err(|_| "flow/zip pair type is invalid")?
            .profile()
            .map_err(|_| "flow/zip pair profile is invalid")?
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "flow/zip pair value contract is invalid")?;
    paired_semantic_contract(left, right, paired)
}

/// Pair exact prepared structured schemas with the same temporal laws.
/// Schema profiles and the resulting output contract are selected before Play.
/// This definition alone does not install or authorize a host implementation.
pub fn flow_zip_typed_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let transport_kind = |ty: &StructuredInfoType| match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => Ok(kind.clone()),
        _ => ty.profile().map(|profile| profile.value_kind().clone()),
    };
    if transport_kind(left_type).map_err(|_| "flow/zip left schema is invalid")? != left.value_kind
        || transport_kind(right_type).map_err(|_| "flow/zip right schema is invalid")?
            != right.value_kind
    {
        return Err("flow/zip schema differs from its exact input value contract");
    }
    if (left.maximum_bytes == 0 && left.value_kind.as_str() != EMPTY_INFO_ID)
        || (right.maximum_bytes == 0 && right.value_kind.as_str() != EMPTY_INFO_ID)
    {
        return Err("flow/zip requires finite canonical input envelopes");
    }
    let encoder = PreparedTypedTuplePairEncoder::new(
        left_type.clone(),
        left.maximum_bytes,
        right_type.clone(),
        right.maximum_bytes,
    )
    .map_err(|_| "flow/zip typed pair exceeds structured Info bounds")?;
    let paired = CheckedValueContract::new(
        encoder
            .value_type()
            .profile()
            .map_err(|_| "flow/zip pair profile is invalid")?
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "flow/zip pair value contract is invalid")?;
    paired_semantic_contract(left, right, paired)
}

fn paired_semantic_contract(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    paired: CheckedValueContract,
) -> Result<Kind, &'static str> {
    let input = |name: &str, contract: &CheckedValueContract| PortDescriptor {
        port_id: port_id(name),
        value_kind: contract.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let output = PortDescriptor {
        port_id: port_id("paired"),
        value_kind: paired.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let terminal = CheckedValueContract::new(
        kind_id(TERMINAL_INFO_ID),
        TERMINAL_INFO_ENCODED_LEN as u32,
        vec![],
    )
    .expect("canonical terminal info has one exact finite envelope");
    let value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("left")),
            contract: left.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("right")),
            contract: right.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("paired")),
            contract: paired.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::InputAbnormal(port_id("left")),
            contract: terminal.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::InputAbnormal(port_id("right")),
            contract: terminal.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::OutputAbnormal(port_id("paired")),
            contract: terminal,
        },
    ];
    let terminal_profile = |name: &str| {
        KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
            input_port_id: port_id(name),
            output_port_id: port_id("paired"),
            normal_close: NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: paired.maximum_bytes,
            }),
            abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
            cancellation: CancellationTransduction::NotCancellable,
        })
    };
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_ZIP_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_ZIP_CONTRACT_REVISION),
        inputs: vec![input("left", left), input("right", right)],
        outputs: vec![output],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(value_contracts),
            terminal_profile("left"),
            terminal_profile("right"),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 3,
            max_queue_bytes: left
                .maximum_bytes
                .checked_add(right.maximum_bytes)
                .and_then(|bytes| bytes.checked_add(paired.maximum_bytes))
                .and_then(|bytes| bytes.checked_add(TERMINAL_INFO_ENCODED_LEN as u32))
                .ok_or("flow/zip finite queue envelope overflows")?,
        },
    })
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_zip_kind(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_ZIP_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(flow_zip_semantic_contract(left, right).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_is_exact_bounded_and_terminal_per_input() {
        let left = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let right = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let contract = flow_zip_semantic_contract(&left, &right).unwrap();
        assert_eq!(contract.inputs[0].value_kind, left.value_kind);
        assert_eq!(contract.inputs[1].value_kind, right.value_kind);
        assert_ne!(contract.outputs[0].value_kind, left.value_kind);
        assert_eq!(contract.terminal_transductions().count(), 2);
        assert!(contract.terminal_transductions().all(|profile| matches!(
            profile.normal_close,
            NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                maximum_items: 1,
                ..
            })
        )));
    }

    #[test]
    fn typed_pair_contract_retains_exact_schema_and_output_profile() {
        let primitive = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
        let packet = StructuredInfoType::record(
            kind_id("type/Packet@1"),
            vec![conduit_core::StructuredFieldType::new("count", primitive.clone()).unwrap()],
        )
        .unwrap();
        let left =
            CheckedValueContract::new(packet.profile().unwrap().value_kind().clone(), 256, vec![])
                .unwrap();
        let right = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let typed = flow_zip_typed_semantic_contract(&left, &packet, &right, &primitive).unwrap();
        let encoder =
            PreparedTypedTuplePairEncoder::new(packet.clone(), 256, primitive.clone(), 8).unwrap();
        assert_eq!(
            &typed.outputs[0].value_kind,
            encoder.value_type().profile().unwrap().value_kind()
        );
        assert_ne!(
            typed.outputs[0].value_kind,
            flow_zip_semantic_contract(&left, &right).unwrap().outputs[0].value_kind
        );
        assert_eq!(typed.terminal_transductions().count(), 2);
        assert!(flow_zip_typed_semantic_contract(&right, &packet, &right, &primitive).is_err());
    }
}
