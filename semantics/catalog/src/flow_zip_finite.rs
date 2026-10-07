//! Exact structured pairing for payload-only finite protocol streams.
use conduit_core::*;

pub const FLOW_ZIP_FINITE_KIND: &str = "flow/zip/finite";
pub const FLOW_ZIP_FINITE_REVISION: &str = "conduit.flow/zip-finite@1";
pub const FLOW_ZIP_FINITE_SPECIALIZED_REVISION: &str = "conduit.flow/zip-finite-specialized@1";
pub const FLOW_ZIP_FEEDBACK_SPECIALIZED_REVISION: &str = "conduit.flow/zip-feedback-specialized@1";

/// Distinct exact schema/contract specializations may coexist in one Source
/// catalog. Identity is data-derived; it never names a domain or execution order.
pub fn flow_zip_finite_specialized_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    specialize_pair(
        flow_zip_finite_semantic_contract(left, left_type, right, right_type)?,
        left_type,
        right_type,
        FLOW_ZIP_FINITE_KIND,
        FLOW_ZIP_FINITE_SPECIALIZED_REVISION,
    )
}

pub fn flow_zip_feedback_specialized_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    specialize_pair(
        flow_zip_feedback_semantic_contract(left, left_type, right, right_type)?,
        left_type,
        right_type,
        FLOW_ZIP_FEEDBACK_KIND,
        FLOW_ZIP_FEEDBACK_SPECIALIZED_REVISION,
    )
}

fn specialize_pair(
    mut kind: Kind,
    left: &StructuredInfoType,
    right: &StructuredInfoType,
    family: &str,
    revision: &str,
) -> Result<Kind, &'static str> {
    kind.validate()
        .map_err(|_| "invalid finite pair contract")?;
    let mut bytes = alloc::vec::Vec::new();
    bytes.extend_from_slice(&left.semantic_digest().map_err(|_| "invalid left schema")?);
    bytes.extend_from_slice(
        &right
            .semantic_digest()
            .map_err(|_| "invalid right schema")?,
    );
    bytes.extend_from_slice(compute_checked_front_fingerprint(&kind.checked_front()).as_bytes());
    let digest = semantic_digest(revision, &bytes);
    let mut hexadecimal = alloc::string::String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        hexadecimal.push(char::from(HEX[usize::from(byte >> 4)]));
        hexadecimal.push(char::from(HEX[usize::from(byte & 15)]));
    }
    kind.kind_id = kind_id(&alloc::format!("{family}/typed-{hexadecimal}"));
    kind.kind_contract_revision = KindIdentity::from(revision);
    Ok(kind)
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_zip_finite_specialized_kind(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<KindId, alloc::string::String> {
    let kind = flow_zip_finite_specialized_semantic_contract(left, left_type, right, right_type)
        .map_err(alloc::string::String::from)?;
    let identity = kind.kind_id.clone();
    install_pair_kind(kind, left, left_type, right, right_type, startup, profile)?;
    Ok(identity)
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_zip_feedback_specialized_kind(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<KindId, alloc::string::String> {
    let kind = flow_zip_feedback_specialized_semantic_contract(left, left_type, right, right_type)
        .map_err(alloc::string::String::from)?;
    let identity = kind.kind_id.clone();
    install_pair_kind(kind, left, left_type, right, right_type, startup, profile)?;
    Ok(identity)
}

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
    install_pair_kind(kind, left, left_type, right, right_type, startup, profile)
}

pub const FLOW_ZIP_FEEDBACK_KIND: &str = "flow/zip/feedback";

/// Each published pair requires exactly one subsequent left state return.
/// Right closure retains a pending event until that return arrives and waits
/// for the last pair's state return before closing. Left loss remains closure.
pub fn flow_zip_feedback_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let mut kind = flow_zip_finite_semantic_contract(left, left_type, right, right_type)?;
    kind.kind_id = kind_id(FLOW_ZIP_FEEDBACK_KIND);
    kind.kind_contract_revision = KindIdentity::from("conduit.flow/zip-feedback@1");
    Ok(kind)
}

#[cfg(feature = "plot-catalog")]
pub fn install_flow_zip_feedback_kind(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = flow_zip_feedback_semantic_contract(left, left_type, right, right_type)
        .map_err(alloc::string::String::from)?;
    install_pair_kind(kind, left, left_type, right, right_type, startup, profile)
}

#[cfg(feature = "plot-catalog")]
fn install_pair_kind(
    kind: Kind,
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let encoder = PreparedTypedTuplePairEncoder::new(
        left_type.clone(),
        left.maximum_bytes,
        right_type.clone(),
        right.maximum_bytes,
    )
    .map_err(|error| alloc::format!("{error:?}"))?;
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: alloc::vec::Vec::new(),
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    startup.insert_structured_type(
        alloc::format!("{}/paired", kind.kind_id.as_str()),
        encoder.value_type().clone(),
    )?;
    profile
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_specialization_identity_tracks_schema_order_bounds_constraints_and_mode() {
        let u64_type = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let u8_type = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let left = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let right = CheckedValueContract::new(kind_id("value/u8"), 1, alloc::vec![]).unwrap();
        let first =
            flow_zip_finite_specialized_semantic_contract(&left, &u64_type, &right, &u8_type)
                .unwrap();
        let again =
            flow_zip_finite_specialized_semantic_contract(&left, &u64_type, &right, &u8_type)
                .unwrap();
        assert_eq!(first, again);
        first.validate().unwrap();
        let reversed =
            flow_zip_finite_specialized_semantic_contract(&right, &u8_type, &left, &u64_type)
                .unwrap();
        assert_ne!(first.kind_id, reversed.kind_id);
        let wider = CheckedValueContract::new(kind_id("value/u64"), 9, alloc::vec![]).unwrap();
        assert_ne!(
            first.kind_id,
            flow_zip_finite_specialized_semantic_contract(&wider, &u64_type, &right, &u8_type)
                .unwrap()
                .kind_id
        );
        let constrained = CheckedValueContract::new(
            kind_id("value/u64"),
            8,
            alloc::vec![ValueConstraint::ByteLength {
                minimum: 8,
                maximum: 8
            }],
        )
        .unwrap();
        assert_ne!(
            first.kind_id,
            flow_zip_finite_specialized_semantic_contract(
                &constrained,
                &u64_type,
                &right,
                &u8_type
            )
            .unwrap()
            .kind_id
        );
        let feedback =
            flow_zip_feedback_specialized_semantic_contract(&left, &u64_type, &right, &u8_type)
                .unwrap();
        assert_ne!(first.kind_id, feedback.kind_id);
        assert_eq!(first.semantic_laws, feedback.semantic_laws);
        assert_eq!(
            first.kind_contract_revision.as_str(),
            FLOW_ZIP_FINITE_SPECIALIZED_REVISION
        );
        assert!(
            flow_zip_finite_specialized_semantic_contract(&left, &u8_type, &right, &u8_type)
                .is_err()
        );
    }

    #[cfg(feature = "plot-catalog")]
    #[test]
    fn distinct_specialized_pair_schemas_coexist_in_one_source_catalog() {
        use conduit_plot::*;
        let u64_type = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let u8_type = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let u64_contract =
            CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let u8_contract = CheckedValueContract::new(kind_id("value/u8"), 1, alloc::vec![]).unwrap();
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        let first = install_flow_zip_finite_specialized_kind(
            &u64_contract,
            &u64_type,
            &u8_contract,
            &u8_type,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        let second = install_flow_zip_finite_specialized_kind(
            &u8_contract,
            &u8_type,
            &u64_contract,
            &u64_type,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        assert_ne!(first, second);
        let source = alloc::format!("with {}/paired as First\nwith {}/paired as Second\nplot first (\n query: First...| >> result: U64...|\n) = (.0)\nplot second (\n query: Second...| >> result: U64...|\n) = (.1)\n",first.as_str(),second.as_str());
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        for (name, left_type, left, right_type, right) in [
            (
                "first",
                u64_type.clone(),
                42_u64.to_le_bytes().to_vec(),
                u8_type.clone(),
                alloc::vec![7],
            ),
            (
                "second",
                u8_type.clone(),
                alloc::vec![7],
                u64_type.clone(),
                42_u64.to_le_bytes().to_vec(),
            ),
        ] {
            let expanded = expand_canonical_plot_for_authoring(&checked, name, &profile).unwrap();
            let ConfigurationValue::Text(encoded) =
                &expanded.expanded.gears[0].configuration[0].value
            else {
                panic!("expression")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let mut pair = PreparedTypedTuplePairEncoder::new(
                left_type,
                left.len() as u32,
                right_type,
                right.len() as u32,
            )
            .unwrap();
            let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
            assert_eq!(
                evaluator
                    .evaluate(pair.encode(&left, &right).unwrap())
                    .unwrap(),
                42_u64.to_le_bytes()
            );
        }
    }
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
