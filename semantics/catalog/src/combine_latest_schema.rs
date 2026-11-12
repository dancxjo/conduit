//! Canonical exact specializations of existing latest-value state semantics.
//! No new runtime state machine or alternative pairing representation.
use alloc::{format, vec::Vec};
use conduit_core::{CheckedValueContract, Kind, KindId, KindIdentity};
pub fn combine_latest_schema_semantic_contract(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<Kind, &'static str> {
    combine_latest_schema_with_input_terminals(left, right, [true, true])
}
/// Exact input terminal capabilities are part of canonical Kind identity.
/// Output terminal truth remains explicit; absent input terminals are refused.
pub fn combine_latest_schema_with_input_terminals(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    input_terminals: [bool; 2],
) -> Result<Kind, &'static str> {
    use conduit_core::{AbnormalTerminalTransduction, FrontValueLocation, KindSemanticLaw};
    let mut kind = super::combine_latest_semantic_contract(left, right)?;
    for (port, accepts) in kind.inputs.iter_mut().zip(input_terminals) {
        if !accepts {
            port.abnormal_kind = None;
            for law in &mut kind.semantic_laws {
                match law {
                    KindSemanticLaw::ValueContracts(values) => values.retain(|entry| {
                        entry.location != FrontValueLocation::InputAbnormal(port.port_id.clone())
                    }),
                    KindSemanticLaw::TerminalTransduction(profile)
                        if profile.input_port_id == port.port_id =>
                    {
                        profile.abnormal = AbnormalTerminalTransduction::NotAccepted
                    }
                    _ => {}
                }
            }
        }
    }
    kind.validate()
        .map_err(|_| "invalid exact latest-value contracts")?;
    let mut identity = Vec::new();
    let revision = kind.kind_contract_revision.as_str().as_bytes();
    identity.extend_from_slice(&(revision.len() as u32).to_le_bytes());
    identity.extend_from_slice(revision);
    for contract in [left, right] {
        identity.extend_from_slice(&(contract.identity_byte_len() as u32).to_le_bytes());
        contract.visit_identity_bytes(|bytes| identity.extend_from_slice(bytes));
    }
    // Preserve the existing Some/Some canonical family exactly.
    if input_terminals != [true, true] {
        identity.extend_from_slice(&[
            0xff,
            u8::from(input_terminals[0]),
            u8::from(input_terminals[1]),
        ]);
    }
    let digest =
        conduit_core::semantic_digest("conduit.state/combine-latest-specialization@1", &identity);
    let suffix = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<alloc::string::String>();
    kind.kind_id = KindId::from(format!("state/combine-latest/schema-{suffix}"));
    kind.kind_contract_revision =
        KindIdentity::from(format!("conduit.state/combine-latest-schema-{suffix}@1"));
    Ok(kind)
}
#[cfg(feature = "plot-catalog")]
pub fn install_combine_latest_schema_kind(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<KindId, alloc::string::String> {
    install_combine_latest_schema_with_input_terminals(left, right, [true, true], startup, profiles)
}
#[cfg(feature = "plot-catalog")]
pub fn install_combine_latest_schema_with_input_terminals(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    input_terminals: [bool; 2],
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<KindId, alloc::string::String> {
    let kind = combine_latest_schema_with_input_terminals(left, right, input_terminals)
        .map_err(alloc::string::String::from)?;
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: Vec::new(),
    })?;
    profiles
        .insert_kind(kind.clone())
        .map_err(|e| format!("{e}"))?;
    Ok(kind.kind_id)
}
#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::*;
    #[test]
    fn input_terminal_capabilities_are_exact_and_identity_bearing() {
        let value = CheckedValueContract::new(kind_id("test/series"), 128, alloc::vec![]).unwrap();
        let both = combine_latest_schema_semantic_contract(&value, &value).unwrap();
        for policy in [[false, false], [true, false], [false, true]] {
            let kind = combine_latest_schema_with_input_terminals(&value, &value, policy).unwrap();
            kind.validate().unwrap();
            assert_ne!(kind.kind_id, both.kind_id);
            assert_eq!(kind.outputs, both.outputs);
            for (port, accepts) in kind.inputs.iter().zip(policy) {
                assert_eq!(port.abnormal_kind.is_some(), accepts);
            }
        }
    }
    #[test]
    fn exact_bounds_constraints_and_pair_shapes_have_distinct_names_without_changing_semantics() {
        let series =
            CheckedValueContract::new(kind_id("test/series"), 1024, alloc::vec![]).unwrap();
        let first = combine_latest_schema_semantic_contract(&series, &series).unwrap();
        let base = super::super::combine_latest_semantic_contract(&series, &series).unwrap();
        assert_eq!(first.semantic_contract(), base.semantic_contract());
        assert_eq!(first.inputs, base.inputs);
        assert_eq!(first.outputs, base.outputs);
        let pair = PreparedTuplePairEncoder::new(
            series.value_kind.clone(),
            1024,
            series.value_kind.clone(),
            1024,
        )
        .unwrap();
        let paired = CheckedValueContract::new(
            pair.value_type()
                .unwrap()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            pair.maximum_bytes(),
            alloc::vec![],
        )
        .unwrap();
        let second = combine_latest_schema_semantic_contract(&paired, &series).unwrap();
        assert_ne!(first.kind_id, second.kind_id);
        let mut narrow = series.clone();
        narrow.maximum_bytes -= 1;
        assert_ne!(
            first.kind_id,
            combine_latest_schema_semantic_contract(&narrow, &series)
                .unwrap()
                .kind_id
        );
        let constrained = CheckedValueContract::new(
            series.value_kind.clone(),
            1024,
            alloc::vec![ValueConstraint::ByteLength {
                minimum: 1,
                maximum: 1024
            }],
        )
        .unwrap();
        assert_ne!(
            first.kind_id,
            combine_latest_schema_semantic_contract(&constrained, &series)
                .unwrap()
                .kind_id
        );
        #[cfg(feature = "plot-catalog")]
        {
            let mut startup = conduit_plot::StartupCatalog::new();
            let mut profiles = conduit_plot::ProfileCatalog::new();
            install_combine_latest_schema_kind(&series, &series, &mut startup, &mut profiles)
                .unwrap();
            install_combine_latest_schema_kind(&paired, &series, &mut startup, &mut profiles)
                .unwrap();
        }
    }
}
