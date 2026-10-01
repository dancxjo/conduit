//! Exact bounded latest-pair state over two independent Flows.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FiniteTerminalEmission, FrontValueContract, FrontValueLocation, Kind,
    KindIdentity, KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection,
    PortTemporal, PreparedTuplePairEncoder, TerminalTransductionProfile, TERMINAL_INFO_ENCODED_LEN,
    TERMINAL_INFO_ID, UNIT_INFO_ID,
};

pub const COMBINE_LATEST_KIND: &str = "state/combine-latest";
pub const COMBINE_LATEST_CONTRACT_REVISION: &str = "conduit.state/combine-latest@1";

/// Specializes `state/combine-latest` over two exact Flow value contracts.
///
/// One latest value per side is retained. No output exists until both sides
/// have produced once; thereafter every accepted replacement owes the exact
/// pair containing that replacement and the other side's committed latest.
pub fn combine_latest_semantic_contract(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    if (left.maximum_bytes == 0 && left.value_kind.as_str() != UNIT_INFO_ID)
        || (right.maximum_bytes == 0 && right.value_kind.as_str() != UNIT_INFO_ID)
    {
        return Err("state/combine-latest requires finite canonical input envelopes");
    }
    let encoder = PreparedTuplePairEncoder::new(
        left.value_kind.clone(),
        left.maximum_bytes,
        right.value_kind.clone(),
        right.maximum_bytes,
    )
    .map_err(|_| "state/combine-latest pair exceeds structured Info bounds")?;
    let latest = CheckedValueContract::new(
        encoder
            .value_type()
            .map_err(|_| "state/combine-latest pair type is invalid")?
            .profile()
            .map_err(|_| "state/combine-latest pair profile is invalid")?
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "state/combine-latest output contract is invalid")?;
    let input = |name: &str, contract: &CheckedValueContract| PortDescriptor {
        port_id: port_id(name),
        value_kind: contract.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let output = PortDescriptor {
        port_id: port_id("latest"),
        value_kind: latest.value_kind.clone(),
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
            location: FrontValueLocation::Output(port_id("latest")),
            contract: latest.clone(),
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
            location: FrontValueLocation::OutputAbnormal(port_id("latest")),
            contract: terminal,
        },
    ];
    let terminal_profile = |name: &str| {
        KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
            input_port_id: port_id(name),
            output_port_id: port_id("latest"),
            normal_close: NormalCloseTransduction::FlushThenPropagateWhenAllClose(
                FiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: latest.maximum_bytes,
                },
            ),
            abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
            cancellation: CancellationTransduction::NotCancellable,
        })
    };
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(COMBINE_LATEST_KIND),
        kind_contract_revision: KindIdentity::from(COMBINE_LATEST_CONTRACT_REVISION),
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
            max_queue_items: 4,
            max_queue_bytes: left
                .maximum_bytes
                .checked_add(right.maximum_bytes)
                .and_then(|bytes| bytes.checked_add(latest.maximum_bytes))
                .and_then(|bytes| bytes.checked_add(TERMINAL_INFO_ENCODED_LEN as u32))
                .ok_or("state/combine-latest finite queue envelope overflows")?,
        },
    })
}

#[cfg(feature = "form-catalog")]
pub fn install_combine_latest_kind(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_form::KindSignature {
        kind: COMBINE_LATEST_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(combine_latest_semantic_contract(left, right).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_retains_one_exact_pair_and_waits_for_all_input_closes() {
        let left = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let right = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let contract = combine_latest_semantic_contract(&left, &right).unwrap();
        assert_eq!(contract.terminal_transductions().count(), 2);
        assert!(contract.terminal_transductions().all(|profile| matches!(
            profile.normal_close,
            NormalCloseTransduction::FlushThenPropagateWhenAllClose(_)
        )));
        assert_eq!(contract.limits.max_queue_items, 4);
    }
}
