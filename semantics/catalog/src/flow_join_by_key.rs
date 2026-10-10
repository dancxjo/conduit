//! Exact finite one-to-one joining of two keyed Flows.

#[cfg(feature = "plot-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, FrontValueContract, FrontValueLocation, KeyedJoinCapacityBehavior,
    KeyedJoinOutputOrder, KeyedJoinPairing, KeyedJoinSemanticLaw, KeyedJoinUnmatchedCloseBehavior,
    Kind, KindIdentity, KindSemanticLaw, NormalCloseTransduction, PortDescriptor, PortDirection,
    PortTemporal, PreparedTuplePairEncoder, PreparedTupleTripleEncoder,
    TerminalTransductionProfile, EMPTY_INFO_ID, TERMINAL_INFO_ENCODED_LEN, TERMINAL_INFO_ID,
};

pub const FLOW_JOIN_BY_KEY_KIND: &str = "flow/join/by-key";
pub const FLOW_JOIN_BY_KEY_CONTRACT_REVISION: &str = "conduit.flow/join/by-key@1";
pub const FLOW_JOIN_BY_KEY_PENDING_PER_SIDE: u16 = 8;

/// Specializes a finite FIFO one-to-one keyed join.
///
/// Inputs are canonical `(key, value)` tuples. Duplicate keys are legal and
/// pair oldest-with-oldest. The arrival which completes a pair determines its
/// output position. Unmatched arrivals apply bounded backpressure once the
/// per-side pending set is full; they are never evicted or guessed together.
pub fn flow_join_by_key_semantic_contract(
    key: &CheckedValueContract,
    left_value: &CheckedValueContract,
    right_value: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    for contract in [key, left_value, right_value] {
        if contract.maximum_bytes == 0 && contract.value_kind.as_str() != EMPTY_INFO_ID {
            return Err("flow/join/by-key requires finite canonical value envelopes");
        }
    }
    let left_pair = PreparedTuplePairEncoder::new(
        key.value_kind.clone(),
        key.maximum_bytes,
        left_value.value_kind.clone(),
        left_value.maximum_bytes,
    )
    .map_err(|_| "flow/join/by-key left tuple exceeds structured Info bounds")?;
    let right_pair = PreparedTuplePairEncoder::new(
        key.value_kind.clone(),
        key.maximum_bytes,
        right_value.value_kind.clone(),
        right_value.maximum_bytes,
    )
    .map_err(|_| "flow/join/by-key right tuple exceeds structured Info bounds")?;
    let joined = PreparedTupleTripleEncoder::new(
        key.value_kind.clone(),
        key.maximum_bytes,
        left_value.value_kind.clone(),
        left_value.maximum_bytes,
        right_value.value_kind.clone(),
        right_value.maximum_bytes,
    )
    .map_err(|_| "flow/join/by-key output tuple exceeds structured Info bounds")?;
    let checked = |encoder: &PreparedTuplePairEncoder| {
        CheckedValueContract::new(
            encoder
                .value_type()
                .map_err(|_| "flow/join/by-key tuple type is invalid")?
                .profile()
                .map_err(|_| "flow/join/by-key tuple profile is invalid")?
                .value_kind()
                .clone(),
            encoder.maximum_bytes(),
            vec![],
        )
        .map_err(|_| "flow/join/by-key tuple contract is invalid")
    };
    let left = checked(&left_pair)?;
    let right = checked(&right_pair)?;
    let output = CheckedValueContract::new(
        joined
            .value_type()
            .map_err(|_| "flow/join/by-key output type is invalid")?
            .profile()
            .map_err(|_| "flow/join/by-key output profile is invalid")?
            .value_kind()
            .clone(),
        joined.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "flow/join/by-key output contract is invalid")?;
    let port = |name: &str, direction, contract: &CheckedValueContract| PortDescriptor {
        port_id: port_id(name),
        value_kind: contract.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let terminal = CheckedValueContract::new(
        kind_id(TERMINAL_INFO_ID),
        TERMINAL_INFO_ENCODED_LEN as u32,
        vec![],
    )
    .expect("canonical terminal info has one exact finite envelope");
    let mut value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("left")),
            contract: left.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("right")),
            contract: right.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("joined")),
            contract: output.clone(),
        },
    ];
    for name in ["left", "right"] {
        value_contracts.push(FrontValueContract {
            location: FrontValueLocation::InputAbnormal(port_id(name)),
            contract: terminal.clone(),
        });
    }
    value_contracts.push(FrontValueContract {
        location: FrontValueLocation::OutputAbnormal(port_id("joined")),
        contract: terminal,
    });
    let terminal_profile = |name: &str| {
        KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
            input_port_id: port_id(name),
            output_port_id: port_id("joined"),
            normal_close: NormalCloseTransduction::PropagateWhenAllClose,
            abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
            cancellation: CancellationTransduction::NotCancellable,
        })
    };
    let pending_bytes = left
        .maximum_bytes
        .checked_add(right.maximum_bytes)
        .and_then(|bytes| bytes.checked_mul(u32::from(FLOW_JOIN_BY_KEY_PENDING_PER_SIDE)))
        .and_then(|bytes| bytes.checked_add(output.maximum_bytes))
        .and_then(|bytes| bytes.checked_add(TERMINAL_INFO_ENCODED_LEN as u32))
        .ok_or("flow/join/by-key pending envelope overflows")?;
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(FLOW_JOIN_BY_KEY_KIND),
        kind_contract_revision: KindIdentity::from(FLOW_JOIN_BY_KEY_CONTRACT_REVISION),
        inputs: vec![
            port("left", PortDirection::Input, &left),
            port("right", PortDirection::Input, &right),
        ],
        outputs: vec![port("joined", PortDirection::Output, &output)],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(value_contracts),
            KindSemanticLaw::KeyedJoin(KeyedJoinSemanticLaw {
                key: key.clone(),
                left_value: left_value.clone(),
                right_value: right_value.clone(),
                maximum_pending_per_side: FLOW_JOIN_BY_KEY_PENDING_PER_SIDE,
                pairing: KeyedJoinPairing::OldestWithOldest,
                output_order: KeyedJoinOutputOrder::MatchCompletionArrival,
                capacity: KeyedJoinCapacityBehavior::BackpressureUnmatched,
                unmatched_on_close:
                    KeyedJoinUnmatchedCloseBehavior::DiscardWhenMatchBecomesImpossible,
            }),
            terminal_profile("left"),
            terminal_profile("right"),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: FLOW_JOIN_BY_KEY_PENDING_PER_SIDE * 2 + 1,
            max_queue_bytes: pending_bytes,
        },
    })
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_join_by_key_kind(
    key: &CheckedValueContract,
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_JOIN_BY_KEY_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(flow_join_by_key_semantic_contract(key, left, right).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialization_is_finite_typed_and_closes_only_after_both_inputs() {
        let key = CheckedValueContract::new(kind_id("value/text"), 32, vec![]).unwrap();
        let left = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let right = CheckedValueContract::new(kind_id("value/bool"), 1, vec![]).unwrap();
        let contract = flow_join_by_key_semantic_contract(&key, &left, &right).unwrap();
        assert_eq!(contract.terminal_transductions().count(), 2);
        assert!(contract.terminal_transductions().all(|profile| matches!(
            profile.normal_close,
            NormalCloseTransduction::PropagateWhenAllClose
        )));
        assert_eq!(
            contract.limits.max_queue_items,
            FLOW_JOIN_BY_KEY_PENDING_PER_SIDE * 2 + 1
        );
    }
}
