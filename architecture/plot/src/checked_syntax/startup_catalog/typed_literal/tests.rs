use super::*;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindSemanticLaw, KindTerminalBehavior,
    PortDescriptor, PortDirection,
};

fn fixture() -> (StartupCatalog, crate::ProfileCatalog, TypedLiteralFamily) {
    let document =
        crate::parse_syntax_document("type FixtureLiteral = {\n    payload: Text <= 64B\n}\n");
    let checked = crate::check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0];
    let mut startup = StartupCatalog::new();
    startup
        .insert_checked_native_type("FixtureLiteral", ty)
        .unwrap();
    startup
        .insert(KindSignature {
            kind: "fixture/literal".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = crate::ProfileCatalog::new();
    profile
        .insert_kind(Kind {
            kind_id: kind_id("fixture/literal"),
            kind_contract_revision: KindIdentity::from("fixture/literal@1"),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: ty.value_type.profile().unwrap().value_kind().clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 16384,
            },
        })
        .unwrap();
    let family = TypedLiteralFamily {
        revision: "fixture/notation@1".into(),
        origin: TypedLiteralFamilyOrigin {
            package_content_digest: [7; 32],
            module_path: "fixture/notation".into(),
            source_document_id: document.source_document_id().clone(),
        },
        branches: vec![TypedLiteralBranch {
            delimiter: TypedLiteralDelimiter::Slash,
            lexical_policy: TypedLiteralLexicalPolicy::RawUnicode,
            parser_contract: "fixture/parser@1".into(),
            constructor_kind: kind_id("fixture/literal"),
            constructor_revision: KindIdentity::from("fixture/literal@1"),
            result_type: ty.value_type.clone(),
            maximum_payload_bytes: 64,
        }],
    };
    (startup, profile, family)
}

#[test]
fn fixed_branches_retain_one_exact_constructor_and_output_type() {
    let (mut startup, profile, mut family) = fixture();
    let mut alias = family.branches[0].clone();
    alias.delimiter = TypedLiteralDelimiter::DoubleSquare;
    family.branches.push(alias);
    let identity = family.identity_bytes().unwrap();
    family.branches.reverse();
    assert_eq!(identity, family.identity_bytes().unwrap());
    startup
        .insert_typed_literal_family("fixture/notation", family.clone(), &profile)
        .unwrap();
    assert_eq!(
        startup.typed_literal_family("fixture/notation"),
        Some(&family)
    );
    assert_eq!(TypedLiteralDelimiter::DoubleSquare.pair(), ('⟦', '⟧'));
}

#[test]
fn duplicate_branch_foreign_result_and_stale_constructor_refuse_transactionally() {
    let (mut startup, profile, family) = fixture();
    let mut duplicate = family.clone();
    duplicate.branches.push(duplicate.branches[0].clone());
    let mut foreign = family.clone();
    foreign.branches[0].result_type = crate::check_syntax_document(
        &crate::parse_syntax_document("type ForeignLiteral = {\n    payload: Text <= 64B\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap()
    .native_types[0]
        .value_type
        .clone();
    let mut stale = family.clone();
    stale.branches[0].constructor_revision = KindIdentity::from("fixture/literal@0");
    let mut absent = family.clone();
    absent.branches[0].constructor_kind = kind_id("fixture/missing");
    for refused in [duplicate, foreign, stale, absent] {
        let before = startup.clone();
        assert!(startup
            .insert_typed_literal_family("fixture/notation", refused, &profile)
            .is_err());
        assert_eq!(startup, before);
    }
}

#[test]
fn constructor_requires_both_installed_startup_and_semantic_truth() {
    let (mut startup, _, family) = fixture();
    let before = startup.clone();
    assert!(startup
        .insert_typed_literal_family("fixture/notation", family, &crate::ProfileCatalog::new())
        .is_err());
    assert_eq!(startup, before);
}

#[test]
fn installed_family_cannot_be_rebound_as_a_type_or_gear() {
    let (mut startup, profile, family) = fixture();
    startup
        .insert_typed_literal_family("fixture/notation", family.clone(), &profile)
        .unwrap();
    let before = startup.clone();
    assert!(startup
        .insert_typed_literal_family("fixture/notation", family.clone(), &profile)
        .is_err());
    assert!(startup
        .insert(KindSignature {
            kind: "fixture/notation".into(),
            startup_parameters: vec![]
        })
        .is_err());
    assert!(startup
        .insert_structured_type("fixture/notation", family.branches[0].result_type.clone())
        .is_err());
    assert!(startup
        .insert_value_kind_alias("fixture/notation", kind_id("value/text"))
        .is_err());
    assert_eq!(startup, before);
}

#[test]
fn source_parser_and_package_revisions_pin_family_identity() {
    let (_, _, family) = fixture();
    let identity = family.identity_bytes().unwrap();
    let mut parser = family.clone();
    parser.branches[0].parser_contract = "fixture/parser@2".into();
    let mut package = family.clone();
    package.origin.package_content_digest[0] ^= 1;
    let mut source = family.clone();
    source.origin.source_document_id = crate::parse_syntax_document("# another exact Source\n")
        .source_document_id()
        .clone();
    let mut revision = family.clone();
    revision.revision = "fixture/notation@2".into();
    for changed in [parser, package, source, revision] {
        assert_ne!(changed.identity_bytes().unwrap(), identity);
    }
}

#[test]
fn payload_identity_and_registry_capacity_have_exact_refusal_boundaries() {
    let (mut startup, profile, family) = fixture();
    let mut excess = family.clone();
    excess.branches[0].maximum_payload_bytes = MAXIMUM_TYPED_LITERAL_PAYLOAD_BYTES + 1;
    let before = startup.clone();
    assert!(startup
        .insert_typed_literal_family("fixture/excess", excess, &profile)
        .is_err());
    assert_eq!(startup, before);
    for index in 0..MAXIMUM_TYPED_LITERAL_FAMILIES {
        startup
            .insert_typed_literal_family(
                format!("fixture/family-{index}"),
                family.clone(),
                &profile,
            )
            .unwrap();
    }
    let before = startup.clone();
    assert!(startup
        .insert_typed_literal_family("fixture/overflow", family.clone(), &profile)
        .is_err());
    assert_eq!(startup, before);
    let mut invalid = family.clone();
    invalid.origin.module_path = "fixture/\u{202e}notation".into();
    assert!(invalid.identity_bytes().is_err());
    invalid = family;
    invalid.branches[0].parser_contract = "x".repeat(MAXIMUM_IDENTITY_BYTES + 1);
    assert!(invalid.identity_bytes().is_err());
}

#[test]
fn a_primitive_result_retains_its_ordinary_info_kind() {
    let (mut startup, profile, mut family) = fixture();
    let mut kind = profile
        .canonical_kind(&kind_id("fixture/literal"))
        .unwrap()
        .clone();
    kind.outputs[0].value_kind = kind_id("value/text");
    let mut primitive_profile = crate::ProfileCatalog::new();
    primitive_profile.insert_kind(kind).unwrap();
    family.branches[0].result_type = StructuredInfoType::leaf(kind_id("value/text")).unwrap();
    startup
        .insert_typed_literal_family(
            "fixture/primitive-notation",
            family.clone(),
            &primitive_profile,
        )
        .unwrap();
    let installed = startup
        .typed_literal_family("fixture/primitive-notation")
        .unwrap();
    assert_eq!(
        installed.branch(TypedLiteralDelimiter::Slash),
        Some(&family.branches[0])
    );
    assert!(installed.branch(TypedLiteralDelimiter::Square).is_none());
}

#[test]
fn serialized_metadata_budget_refuses_before_the_entry_ceiling() {
    let (mut startup, profile, mut family) = fixture();
    let fields = (0..64)
        .map(|index| {
            conduit_core::StructuredFieldType::new(
                format!("field-{index:02}-{}", "x".repeat(119)),
                StructuredInfoType::leaf(kind_id("value/text")).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let large = StructuredInfoType::record(kind_id("fixture/large-result"), fields).unwrap();
    let mut kind = profile
        .canonical_kind(&kind_id("fixture/literal"))
        .unwrap()
        .clone();
    kind.outputs[0].value_kind = large.profile().unwrap().value_kind().clone();
    let mut large_profile = crate::ProfileCatalog::new();
    large_profile.insert_kind(kind).unwrap();
    family.branches[0].result_type = large;
    for delimiter in [
        TypedLiteralDelimiter::Square,
        TypedLiteralDelimiter::Angle,
        TypedLiteralDelimiter::DoubleSquare,
    ] {
        let mut branch = family.branches[0].clone();
        branch.delimiter = delimiter;
        family.branches.push(branch);
    }
    let entry_bytes = family.identity_bytes().unwrap().len() + "fixture/large-00".len();
    let fitting = MAXIMUM_REGISTRY_BYTES / entry_bytes;
    assert!(fitting > 0 && fitting < MAXIMUM_TYPED_LITERAL_FAMILIES);
    for index in 0..fitting {
        startup
            .insert_typed_literal_family(
                format!("fixture/large-{index:02}"),
                family.clone(),
                &large_profile,
            )
            .unwrap();
    }
    let before = startup.clone();
    assert!(startup
        .insert_typed_literal_family(
            format!("fixture/large-{fitting:02}"),
            family,
            &large_profile
        )
        .is_err());
    assert_eq!(startup, before);
}
