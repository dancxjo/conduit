//! A selected Source Back retains its owner's helpers, Types and exact values.
use conduit_core::*;
use conduit_plot::*;
const OWNER: &str = "type OwnedRecord = {\n sample: I32\n}\nplot private (\n >> value: OwnedRecord\n result: OwnedRecord >>\n) = ({sample: .sample + 1})\nplot implementation (\n >> value: OwnedRecord\n result: OwnedRecord >>\n) {\n transform: private()\n value >> transform.value\n transform.result >> result\n}\n";
fn kind(document: &CheckedSyntaxDocument) -> Kind {
    let front = document
        .plots
        .iter()
        .find(|p| p.name == "implementation")
        .unwrap()
        .checked_front();
    Kind {
        startup_parameters: front.startup_parameters().to_vec(),
        shorthand: front.shorthand().map(|(a, b)| (a.clone(), b.clone())),
        kind_id: "fixture/owned-source".into(),
        kind_contract_revision: "fixture/owned-source@1".into(),
        inputs: front.inputs().to_vec(),
        outputs: front.outputs().to_vec(),
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(
            front.value_contracts().to_vec(),
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 4096,
        },
    }
}
fn owner() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(OWNER), &StartupCatalog::new()).unwrap()
}
#[test]
fn source_back_private_helpers_cannot_be_captured_by_the_caller() {
    let owner = owner();
    let definition = kind(&owner);
    let record = owner
        .native_types
        .iter()
        .find(|ty| ty.name == "OwnedRecord")
        .unwrap()
        .value_type
        .clone();
    for alias in ["OwnedRecord", "ImportedRecord"] {
        let mut startup = StartupCatalog::new();
        startup
            .insert_structured_type(alias, record.clone())
            .unwrap();
        startup
            .insert(KindSignature {
                kind: "fixture/owned-source".into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore("fixture/owned-source", definition.checked_front())
            .unwrap();
        let mut profiles = ProfileCatalog::new();
        profiles.insert_kind(definition.clone()).unwrap();
        // The caller's helper calls the Back. It must not replace the owner's
        // same-named helper, which performs the actual +1 operation.
        let caller = format!("plot private (\n >> value: {alias}\n result: {alias} >>\n) {{\n body: fixture/owned-source()\n value >> body.value\n body.result >> result\n}}\nplot implementation (\n >> value: {alias}\n result: {alias} >>\n) {{\n transform: private()\n value >> transform.value\n transform.result >> result\n}}\n");
        let checked = check_syntax_document(&parse_syntax_document(&caller), &startup).unwrap();
        if alias == "OwnedRecord" {
            assert_eq!(
                checked
                    .plots
                    .iter()
                    .find(|p| p.name == "implementation")
                    .unwrap()
                    .checked_plot_id,
                owner
                    .plots
                    .iter()
                    .find(|p| p.name == "implementation")
                    .unwrap()
                    .checked_plot_id,
                "same root identity can have different private helper definitions"
            );
        }
        let mut backs = CanonicalBackCatalog::new();
        backs.insert(&definition, &owner, "implementation").unwrap();
        let expanded = expand_canonical_plot_for_authoring_with_backs(
            &checked,
            "implementation",
            &profiles,
            &backs,
        )
        .unwrap()
        .expanded;
        assert_eq!(expanded.source_document_id, checked.source_document_id);
        assert_eq!(expanded.realization_backs.len(), 1);
        assert_eq!(
            expanded.realization_backs[0].source_document_id,
            owner.source_document_id
        );
        assert_eq!(expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &expanded.gears[0]
            .configuration
            .iter()
            .find(|c| c.key == "program")
            .unwrap()
            .value
        else {
            panic!("pure owned program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert_eq!(program.input_type, record);
        let StructuredInfoTypeShape::Record { fields, .. } = record.shape() else {
            panic!("record")
        };
        let input = StructuredInfoValue::record(
            record.clone(),
            vec![StructuredFieldValue::new(
                "sample",
                StructuredInfoValue::leaf(
                    fields[0].value_type().clone(),
                    8_i32.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let bytes = program.evaluate(&input.canonical_bytes().unwrap()).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let StructuredInfoValueShape::Record(fields) = output.shape() else {
            panic!("record")
        };
        let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
            panic!("I32")
        };
        assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), 9);
    }
}
#[test]
fn refusing_a_duplicate_back_never_retargets_an_admitted_owner() {
    let original = owner();
    let replacement = check_syntax_document(
        &parse_syntax_document(&OWNER.replace(".sample + 1", ".sample + 2")),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(&kind(&original), &original, "implementation")
        .unwrap();
    let admitted = backs
        .checked_definition(&kind_id("fixture/owned-source"))
        .unwrap()
        .clone();
    assert!(matches!(
        backs.insert(&kind(&replacement), &replacement, "implementation"),
        Err(CanonicalBackError::DuplicateKind(_))
    ));
    assert_eq!(
        backs
            .checked_definition(&kind_id("fixture/owned-source"))
            .unwrap(),
        &admitted
    );
}

fn installed(
    owner: &CheckedSyntaxDocument,
) -> (StartupCatalog, ProfileCatalog, CanonicalBackCatalog) {
    let mut startup = StartupCatalog::new();
    let record = owner
        .native_types
        .iter()
        .find(|ty| ty.name == "OwnedRecord")
        .unwrap()
        .value_type
        .clone();
    startup
        .insert_structured_type("ImportedRecord", record)
        .unwrap();
    let definition = kind(owner);
    startup
        .insert(KindSignature {
            kind: "fixture/owned-source".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore("fixture/owned-source", definition.checked_front())
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile.insert_kind(definition.clone()).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs.insert(&definition, owner, "implementation").unwrap();
    (startup, profile, backs)
}
#[test]
fn a_back_does_not_grant_its_private_construction_laws_to_caller_gears() {
    let owner = owner();
    let (startup, profiles, backs) = installed(&owner);
    let caller = "plot forge (\n >> value: ImportedRecord\n result: ImportedRecord >>\n) = ({sample: .sample + 99})\nplot caller (\n >> value: ImportedRecord\n result: ImportedRecord >>\n) {\n trusted: fixture/owned-source()\n foreign: forge()\n value >> trusted.value\n trusted.result >> foreign.value\n foreign.result >> result\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(caller), &startup).unwrap();
    let error =
        expand_canonical_plot_for_authoring_with_backs(&checked, "caller", &profiles, &backs)
            .expect_err("caller has only an imported shape");
    assert_eq!(error.code, "CND-FRM-046");
}
#[test]
fn genuine_source_back_recursion_is_still_refused() {
    let (startup, profiles, _) = installed(&owner());
    let recursive = "plot implementation (\n >> value: ImportedRecord\n result: ImportedRecord >>\n) {\n again: fixture/owned-source()\n value >> again.value\n again.result >> result\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(recursive), &startup).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(&kind(&checked), &checked, "implementation")
        .unwrap();
    let error = expand_canonical_plot_for_authoring_with_backs(
        &checked,
        "implementation",
        &profiles,
        &backs,
    )
    .expect_err("genuine recursive Back");
    assert_eq!(error.code, "CND-FRM-035");
}

#[test]
fn an_explicit_owner_catalog_keeps_private_primitive_contracts_out_of_the_caller() {
    fn scalar(value: &str) -> Kind {
        let port = |name: &str, direction| PortDescriptor {
            port_id: name.into(),
            value_kind: kind_id(value),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "fixture/private".into(),
            kind_contract_revision: "fixture/private@1".into(),
            inputs: vec![port("value", PortDirection::Input)],
            outputs: vec![port("result", PortDirection::Output)],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 8,
            },
        }
    }
    let private = scalar("value/i32");
    let mut owner_startup = StartupCatalog::new();
    owner_startup
        .insert(KindSignature {
            kind: "fixture/private".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    owner_startup
        .insert_fore("fixture/private", private.checked_front())
        .unwrap();
    let source = "plot implementation (\n >> value: I32\n result: I32 >>\n) {\n inner: fixture/private()\n value >> inner.value\n inner.result >> result\n}\n";
    let owner = check_syntax_document(&parse_syntax_document(source), &owner_startup).unwrap();
    let public = kind(&owner);
    let mut owner_profile = ProfileCatalog::new();
    owner_profile.insert_kind(private.clone()).unwrap();
    for conflicting in [false, true] {
        let mut startup = StartupCatalog::new();
        startup
            .insert(KindSignature {
                kind: "fixture/owned-source".into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore("fixture/owned-source", public.checked_front())
            .unwrap();
        let mut caller_profile = ProfileCatalog::new();
        caller_profile.insert_kind(public.clone()).unwrap();
        if conflicting {
            caller_profile.insert_kind(scalar("value/i64")).unwrap();
        }
        let caller = "plot caller (\n >> value: I32\n result: I32 >>\n) {\n body: fixture/owned-source()\n value >> body.value\n body.result >> result\n}\n";
        let checked = check_syntax_document(&parse_syntax_document(caller), &startup).unwrap();
        let mut backs = CanonicalBackCatalog::new();
        backs
            .insert_with_catalog(&public, &owner, "implementation", &owner_profile)
            .unwrap();
        let expanded = expand_canonical_plot_for_authoring_with_backs(
            &checked,
            "caller",
            &caller_profile,
            &backs,
        )
        .unwrap();
        assert_eq!(expanded.expanded.gears.len(), 1);
        assert_eq!(
            expanded.expanded.gears[0].inputs[0].value_kind.as_str(),
            "value/i32"
        );
        let retained = caller_profile.canonical_kind(&kind_id("fixture/private"));
        if conflicting {
            assert_eq!(retained.unwrap().inputs[0].value_kind.as_str(), "value/i64");
        } else {
            assert!(retained.is_none());
        }
    }
}
