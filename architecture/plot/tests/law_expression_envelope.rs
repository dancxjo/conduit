//! Whole capsule identity and independent Source authority, not automatic offers.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
fn fixture() -> (LawExpressionMaterial, LawExpressionContext) {
    let doc = check_syntax_document(
        &parse_syntax_document("type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let witness = doc.native_law_witness("Ordered").unwrap();
    let program = PortableExpressionProgram {
        input_type: witness.checked_type().value_type.clone(),
        input_maximum_bytes: Some(4096),
        output_type: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
        root: PortableExpressionNode {
            value_type: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
            operation: PortableExpressionOperation::Literal("true".into()),
        },
    };
    (
        LawExpressionMaterial {
            program,
            roots: vec![witness.material().unwrap()],
        },
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(witness)).unwrap(),
    )
}
fn bytes(ty: &StructuredInfoType, lo: u8, hi: u8) -> Vec<u8> {
    let field = |name: &str, value: u8| {
        StructuredFieldValue::new(
            name,
            primitive_into_structured(
                StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                &value,
            )
            .unwrap(),
        )
        .unwrap()
    };
    StructuredInfoValue::record(ty.clone(), vec![field("hi", hi), field("lo", lo)])
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn exact_capsule_verifies_only_with_independent_source_and_prepares_real_law_guards() {
    let (material, context) = fixture();
    let encoded = material.canonical_bytes().unwrap();
    assert_eq!(
        LawExpressionMaterial::from_canonical_bytes(&encoded).unwrap(),
        material
    );
    let verified = VerifiedLawExpression::verify_with_context(&encoded, &context).unwrap();
    assert_eq!(verified.program(), &material.program);
    assert_eq!(verified.canonical_bytes(), encoded);
    let mut evaluator = verified.prepare_evaluator().unwrap();
    assert_eq!(
        evaluator
            .evaluate(&bytes(&material.program.input_type, 1, 2))
            .unwrap(),
        &[1]
    );
    assert!(matches!(
        evaluator.evaluate(&bytes(&material.program.input_type, 2, 1)),
        Err(PortableExpressionEvaluationRefusal::NativeLaw(
            PreparedNativeRefusal::Invariant { .. }
        ))
    ));
    let empty = LawExpressionContext::from_source_witnesses(&[]).unwrap();
    assert!(matches!(
        VerifiedLawExpression::verify_with_context(&encoded, &empty),
        Err(LawExpressionRefusal::Context)
    ));
}
#[test]
fn well_framed_omitted_laws_do_not_become_authority_and_alter_the_new_kind() {
    let (material, context) = fixture();
    let mut changed = material.clone();
    changed.roots[0].claimed.invariants.clear();
    let encoded = changed.canonical_bytes().unwrap();
    assert!(matches!(
        VerifiedLawExpression::verify_with_context(&encoded, &context),
        Err(LawExpressionRefusal::Custody(_))
    ));
    let original = law_expression_definition(&material, PortTemporal::Value).unwrap();
    let changed = law_expression_definition(&changed, PortTemporal::Value).unwrap();
    assert_ne!(original.kind_id, changed.kind_id);
    let old = portable_expression_definition(&material.program, PortTemporal::Value).unwrap();
    assert_ne!(original.kind_id, old.kind_id);
    assert_ne!(original.kind_contract_revision, old.kind_contract_revision);
    assert_eq!(original.configuration[0].key, "law-program");
    assert_eq!(old.configuration[0].key, "program");
}
#[test]
fn expression_filter_temporal_and_original_program_have_distinct_exact_identities() {
    let (material, _) = fixture();
    let program = material.program.canonical_bytes().unwrap();
    let expression = law_expression_definition(&material, PortTemporal::Value).unwrap();
    let filter = law_filter_definition(&material, PortTemporal::Value).unwrap();
    let flow = law_filter_definition(&material, PortTemporal::Flow { closes: true }).unwrap();
    assert_ne!(expression.kind_id, filter.kind_id);
    assert_ne!(filter.kind_id, flow.kind_id);
    assert_ne!(filter.outputs[0].value_kind, filter.inputs[0].value_kind);
    assert_eq!(flow.outputs[0].value_kind, flow.inputs[0].value_kind);
    assert_eq!(material.program.canonical_bytes().unwrap(), program);
    assert!(law_expression_definition(&material, PortTemporal::Current).is_err());
}
#[test]
fn malformed_profile_trailing_and_truncated_capsules_refuse_before_source_replay() {
    let (material, context) = fixture();
    let encoded = material.canonical_bytes().unwrap();
    for n in [0, 1, 31, encoded.len() - 1] {
        assert!(VerifiedLawExpression::verify_with_context(&encoded[..n], &context).is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(VerifiedLawExpression::verify_with_context(&trailing, &context).is_err());
    let mut unknown = encoded;
    let index = b"conduit.law-expression.program.v1\0".len() + 4;
    unknown[index] ^= 1;
    assert!(matches!(
        VerifiedLawExpression::verify_with_context(&unknown, &context),
        Err(LawExpressionRefusal::Profile)
    ));
}

#[test]
fn aggregate_module_pressure_is_checked_before_program_or_source_decoding() {
    fn frame(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    fn node(children: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0; 7 * 4];
        out.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for child in children {
            frame(&mut out, &[]);
            out.extend_from_slice(child);
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }
    let mut material = b"conduit.native-law-material.v1\0".to_vec();
    material.extend_from_slice(&node(&vec![node(&[]); 128]));
    let mut encoded = b"conduit.law-expression.program.v1\0".to_vec();
    frame(&mut encoded, LAW_EXPRESSION_PROFILE.as_bytes());
    frame(&mut encoded, &[]); // Invalid program must not be decoded first.
    encoded.extend_from_slice(&2u32.to_le_bytes());
    frame(&mut encoded, &material);
    frame(&mut encoded, &material);
    assert!(matches!(
        LawExpressionMaterial::from_canonical_bytes(&encoded),
        Err(LawExpressionRefusal::Capacity)
    ));
}

#[test]
fn packet_family_source_does_not_replace_the_independent_original_package_anchor() {
    fn witness(comment: &str) -> NativeLawWitness {
        let manifest = "pack capsule/family (\n version = 1.0.0\n) {\n ship Box\n}\n";
        let source = format!("# {comment}\ntype Box<T> = {{\n value: T\n}}\n");
        let syntax = parse_syntax_document(manifest);
        let members = [PackageMemberSource {
            path: "box",
            source: &source,
        }];
        let bundle =
            CheckedPackageBundle::from_sources(manifest, &syntax.packages[0], &members).unwrap();
        let exports =
            PackageExportCatalog::from_bundle(&bundle, manifest, &syntax.packages[0], &members)
                .unwrap();
        let mut startup = StartupCatalog::new();
        exports.install_shipped_types(&mut startup).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(
                "with capsule/family/Box as Imported\ntype Value = Imported<U64>\n",
            ),
            &startup,
        )
        .unwrap();
        checked.native_law_witness("Value").unwrap().clone()
    }
    let original = witness("original");
    let foreign = witness("foreign");
    assert_eq!(original.checked_type(), foreign.checked_type());
    assert_ne!(
        original.family_anchors().unwrap(),
        foreign.family_anchors().unwrap()
    );
    let context =
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(&original)).unwrap();
    let mut material = fixture().0;
    material.program.input_type = original.checked_type().value_type.clone();
    material.roots = vec![original.material().unwrap()];
    assert!(VerifiedLawExpression::verify_with_context(
        &material.canonical_bytes().unwrap(),
        &context
    )
    .is_ok());
    material.roots = vec![foreign.material().unwrap()];
    assert!(matches!(
        VerifiedLawExpression::verify_with_context(&material.canonical_bytes().unwrap(), &context),
        Err(LawExpressionRefusal::Custody(_))
    ));
}

#[test]
fn unchanged_root_claim_cannot_authorize_an_extra_packet_defined_native_owner() {
    let (mut material, context) = fixture();
    let expected = material.program.input_type.clone();
    material.roots[0]
        .source
        .push_str("type Extra = {\n value: U8\n where .value == 9\n}\n");
    // Control: replaying just Ordered still agrees with its expected semantic
    // Type and laws. That is not authority over every declaration in the module.
    assert!(NativeLawWitness::verify(&material.roots[0], &expected).is_ok());
    let changed = check_syntax_document(
        &parse_syntax_document(&material.roots[0].source),
        &StartupCatalog::new(),
    )
    .unwrap();
    material.program.input_type = changed
        .native_types
        .iter()
        .find(|ty| ty.name == "Extra")
        .unwrap()
        .value_type
        .clone();
    let encoded = material.canonical_bytes().unwrap();
    assert!(matches!(
        VerifiedLawExpression::verify_with_context(&encoded, &context),
        Err(LawExpressionRefusal::Custody(
            NativeLawCustodyRefusal::Source
        ))
    ));
}
#[test]
fn capsule_hex_is_strict_and_bounded() {
    let (material, _) = fixture();
    let encoded: String = material
        .canonical_bytes()
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        LawExpressionMaterial::from_canonical_hex(&encoded).unwrap(),
        material
    );
    for invalid in ["0", "gg", "AA"] {
        assert_eq!(
            LawExpressionMaterial::from_canonical_hex(invalid),
            Err(LawExpressionRefusal::Encoding)
        );
    }
    assert_eq!(
        LawExpressionMaterial::from_canonical_hex(
            &"0".repeat(MAXIMUM_LAW_EXPRESSION_BYTES * 2 + 2)
        ),
        Err(LawExpressionRefusal::Capacity)
    );
}
#[test]
fn compiler_emits_original_program_with_independent_opaque_source_context() {
    let source = "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> value: Ordered\n result: Boolean >>\n) = (.lo <= .hi)\n";
    let mut checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let original = checked.native_law_witness("Ordered").unwrap().clone();
    // Public claim tables are deliberately mutable and do not grant custody.
    checked.native_types.clear();
    let mut catalog = ProfileCatalog::new();
    catalog.enable_checked_native_law_expressions();
    let authored = expand_canonical_plot_for_authoring(&checked, "arithmetic", &catalog).unwrap();
    let gear = &authored.expanded.gears[0];
    assert!(gear
        .kind_id
        .as_str()
        .starts_with("conduitese/law-expression/"));
    let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
        panic!("capsule");
    };
    let material = LawExpressionMaterial::from_canonical_hex(encoded).unwrap();
    assert_eq!(material.roots, vec![original.material().unwrap()]);
    assert!(material.program.requires_native_law_custody());
    let context = checked
        .expression_law_context_for(&[original.checked_type().identity.clone()])
        .unwrap();
    VerifiedLawExpression::verify_with_context(&material.canonical_bytes().unwrap(), &context)
        .unwrap()
        .prepare_evaluator()
        .unwrap();
    assert!(checked
        .expression_law_context_for(&[kind_id("foreign/type")])
        .is_err());
    let scalar = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let safe = PortableExpressionProgram {
        input_type: scalar.clone(),
        input_maximum_bytes: Some(1),
        output_type: scalar.clone(),
        root: PortableExpressionNode {
            value_type: scalar,
            operation: PortableExpressionOperation::Input,
        },
    };
    assert!(!safe.requires_native_law_custody());
    let mut forged = material.program.clone();
    let StructuredInfoTypeShape::Record { fields, .. } = material.program.input_type.shape() else {
        panic!("record");
    };
    forged.input_type =
        StructuredInfoType::record(kind_id("conduit.core/record/forged"), fields.to_vec()).unwrap();
    assert!(forged.requires_native_law_custody());
}
