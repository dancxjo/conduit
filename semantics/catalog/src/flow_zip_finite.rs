//! Exact structured pairing for payload-only finite protocol streams.
use conduit_core::*;

pub const FLOW_ZIP_FINITE_KIND: &str = "flow/zip/finite";
pub const FLOW_ZIP_FINITE_REVISION: &str = "conduit.flow/zip-finite@1";

/// Pair one pending value from each input. Closure flushes at most the pair
/// already formed, discards an unmatched value, then closes the output.
/// Refusal and loss remain explicit payload values rather than abnormal lanes.
pub fn flow_zip_finite_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let mut kind = super::flow_zip_typed_semantic_contract(left, left_type, right, right_type)?;
    kind.kind_id = kind_id(FLOW_ZIP_FINITE_KIND);
    kind.kind_contract_revision = KindIdentity::from(FLOW_ZIP_FINITE_REVISION);
    for port in kind.inputs.iter_mut().chain(&mut kind.outputs) {
        port.abnormal_kind = None;
    }
    for law in &mut kind.semantic_laws {
        match law {
            KindSemanticLaw::ValueContracts(contracts) => contracts.retain(|entry| {
                matches!(
                    entry.location,
                    FrontValueLocation::Input(_) | FrontValueLocation::Output(_)
                )
            }),
            KindSemanticLaw::TerminalTransduction(profile) => {
                profile.abnormal = AbnormalTerminalTransduction::NotAccepted;
            }
            _ => {}
        }
    }
    Ok(kind)
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_zip_finite_kind(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = flow_zip_finite_semantic_contract(left, left_type, right, right_type)
        .map_err(alloc::string::String::from)?;
    let encoder = PreparedTypedTuplePairEncoder::new(
        left_type.clone(),
        left.maximum_bytes,
        right_type.clone(),
        right.maximum_bytes,
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    startup.insert(conduit_plot::KindSignature {
        kind: FLOW_ZIP_FINITE_KIND.into(),
        startup_parameters: alloc::vec::Vec::new(),
    })?;
    startup.insert_fore(FLOW_ZIP_FINITE_KIND, kind.checked_front())?;
    startup.insert_structured_type("flow/zip/finite/paired", encoder.value_type().clone())?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_typed_pairing_preserves_bounds_and_flush_contract_without_abnormal_lanes() {
        let word = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let contract = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let kind = flow_zip_finite_semantic_contract(&contract, &word, &contract, &word).unwrap();
        kind.validate().unwrap();
        assert!(kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .all(|port| port.temporal == PortTemporal::Flow { closes: true }
                && port.abnormal_kind.is_none()));
        let encoder = PreparedTypedTuplePairEncoder::new(word.clone(), 8, word.clone(), 8).unwrap();
        for profile in kind.terminal_transductions() {
            assert_eq!(
                profile.normal_close,
                NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: encoder.maximum_bytes()
                })
            );
            assert_eq!(profile.abnormal, AbnormalTerminalTransduction::NotAccepted);
        }
        let original =
            super::super::flow_zip_typed_semantic_contract(&contract, &word, &contract, &word)
                .unwrap();
        assert!(original
            .inputs
            .iter()
            .chain(&original.outputs)
            .all(|port| port.abnormal_kind.is_some()));
        let other = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        assert!(flow_zip_finite_semantic_contract(&contract, &other, &contract, &word).is_err());
    }
    #[cfg(feature = "plot-catalog")]
    #[test]
    fn finite_pair_output_is_an_importable_exact_source_type() {
        use conduit_plot::*;
        let word = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let contract = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_flow_zip_finite_kind(
            &contract,
            &word,
            &contract,
            &word,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        let checked = check_syntax_document(&parse_syntax_document(
            "with flow/zip/finite/paired as Joined\nplot add (\n query: Joined...| >> result: U64...|\n) = (.0 + .1)\n"
        ), &startup).unwrap();
        let expanded = expand_canonical_plot_for_authoring(&checked, "add", &profile).unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("expression");
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let mut encoder = PreparedTypedTuplePairEncoder::new(word.clone(), 8, word, 8).unwrap();
        assert_eq!(&program.input_type, encoder.value_type());
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(
            evaluator
                .evaluate(
                    encoder
                        .encode(&16_u64.to_le_bytes(), &26_u64.to_le_bytes())
                        .unwrap()
                )
                .unwrap(),
            &42_u64.to_le_bytes()
        );
    }
}
