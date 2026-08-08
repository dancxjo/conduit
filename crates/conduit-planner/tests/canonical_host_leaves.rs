use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ConnectionProvider, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, KindContractRevision, OfferGeneration, PortDescriptor,
    PortDirection, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, OperationSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical, PlannerError};
use std::fs;
use std::path::Path;

const VALUE_KIND: &str = "test/text";

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(VALUE_KIND),
        direction,
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    startup
        .insert(OperationSignature {
            operation: "text/source".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert(OperationSignature {
            operation: "text/join".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "prefix".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
        })
        .unwrap();
    startup
        .insert(OperationSignature {
            operation: "presentation/text".into(),
            startup_parameters: vec![],
        })
        .unwrap();

    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("text/source"),
            kind_contract_revision: KindContractRevision::from("text/source@1"),
            inputs: vec![],
            outputs: vec![port("text", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("text/join"),
            kind_contract_revision: KindContractRevision::from("text/join@1"),
            inputs: vec![port("text", PortDirection::Input)],
            outputs: vec![port("text", PortDirection::Output)],
            configuration: vec![ConfigurationField {
                key: "prefix".into(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 8,
                },
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("presentation/text"),
            kind_contract_revision: KindContractRevision::from("presentation/text@1"),
            inputs: vec![port("text", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let source = r#"form greet (
    prefix: Count = 1
    name: test/text > text: test/text
) {
    join: text/join(prefix)
    name > join > text
}

form welcome {
    source: text/source
    hello: greet(2)
    show: presentation/text
    source > hello > show
}
"#;
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).expect("canonical source checks");
    expand_canonical_form(&checked, "welcome", &profile).expect("reusable form expands")
}

fn offer(definition: &KindDefinition) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    CapabilityOffer {
        startup_parameters: definition
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.default_value {
                    ConfigurationValue::Bool(_) => "Boolean",
                    ConfigurationValue::U64(_) => "Count",
                }
                .into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(slug.as_str()),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        execution_profile_id: ExecutionProfileId::from("test/profile"),
        implementation_id: ImplementationId::from(format!("std/{slug}")),
        artifact_id: ArtifactId::from(format!("test/{slug}")),
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 16,
            max_queue_bytes: 1_024,
        },
    }
}

fn host() -> HostAdvertisement {
    let (_, profile) = catalogs();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-host"),
        boot_id: BootId::from("std-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/host"),
        resources: vec![],
        capabilities: ["text/source", "text/join", "presentation/text"]
            .into_iter()
            .map(|kind| offer(profile.get(&kind_id(kind)).unwrap()))
            .collect(),
        planner_capabilities: vec![],
    }
}

#[test]
fn nested_form_terminates_only_in_exact_planned_host_operation_leaves() {
    let expanded = expanded();
    assert_eq!(
        expanded
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<Vec<_>>(),
        ["welcome/hello/join", "welcome/show", "welcome/source"]
    );
    assert!(expanded
        .provenance
        .iter()
        .any(|row| row.source_form == "greet" && row.form_path == ["welcome", "hello"]));
    let join = expanded
        .operations
        .iter()
        .find(|operation| operation.kind_id.as_str() == "text/join")
        .unwrap();
    assert_eq!(join.configuration[0].value, ConfigurationValue::U64(2));

    let host = host();
    let placements = default_expanded_placements(&expanded, std::slice::from_ref(&host))
        .expect("every expanded leaf has an exact offer");
    let plan = plan_expanded_canonical(
        &expanded,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionProvider::Local],
    )
    .expect("ordinary planner seals expanded leaves");

    let planned = &plan.fragments[0].placements;
    assert_eq!(planned.len(), expanded.operations.len());
    assert_eq!(
        planned
            .iter()
            .map(|operation| {
                (
                    operation.kind_id.as_str(),
                    operation.implementation_id.as_str(),
                    operation.artifact_id.as_str(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("text/join", "std/text-join", "test/text-join"),
            (
                "presentation/text",
                "std/presentation-text",
                "test/presentation-text"
            ),
            ("text/source", "std/text-source", "test/text-source"),
        ]
    );
    assert!(planned
        .iter()
        .all(|operation| operation.host_id == host.host_id));
}

#[test]
fn equal_face_with_different_name_and_revision_is_compatible() {
    let expanded = expanded();
    let mut wrong_kind = host();
    let join = wrong_kind
        .capabilities
        .iter_mut()
        .find(|capability| capability.kind_id.as_str() == "text/join")
        .unwrap();
    join.kind_id = kind_id("text/coincident-shape");
    let placements = default_expanded_placements(&expanded, std::slice::from_ref(&wrong_kind))
        .expect("different nominal operation with the same face is compatible");
    let plan = plan_expanded_canonical(
        &expanded,
        std::slice::from_ref(&wrong_kind),
        &placements,
        &[ConnectionProvider::Local],
    )
    .unwrap();
    assert!(plan.fragments[0].placements.iter().any(|placement| {
        placement.kind_id.as_str() == "text/coincident-shape"
            && placement.implementation_id.as_str() == "std/text-join"
    }));

    let mut wrong_revision = host();
    wrong_revision
        .capabilities
        .iter_mut()
        .find(|capability| capability.kind_id.as_str() == "text/join")
        .unwrap()
        .kind_contract_revision = KindContractRevision::from("text/join@2");
    default_expanded_placements(&expanded, &[wrong_revision])
        .expect("face-preserving revision is compatible");
}

#[test]
fn same_name_with_a_different_face_is_incompatible() {
    let expanded = expanded();
    let mut changed_face = host();
    changed_face
        .capabilities
        .iter_mut()
        .find(|capability| capability.kind_id.as_str() == "text/join")
        .unwrap()
        .inputs[0]
        .value_kind = kind_id("test/other-text");
    assert_eq!(
        default_expanded_placements(&expanded, &[changed_face]).unwrap_err(),
        PlannerError::UnknownCapability("text/join".into())
    );
}

#[test]
fn uncatalogued_native_escape_fails_before_planning() {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document("form main {\n escape: native/callback(\"symbol\")\n}\n");
    let checked = check_syntax_document(&syntax, &startup).expect_err("unknown operation fails");
    assert!(checked.message.contains("native/callback"));

    let syntax = parse_syntax_document("form main {\n escape: ffi/call\n}\n");
    let mut startup = StartupCatalog::new();
    startup
        .insert(OperationSignature {
            operation: "ffi/call".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = check_syntax_document(&syntax, &startup)
        .expect("a startup signature alone grants no executable realization");
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-037");
}

#[test]
fn canonical_realization_sources_do_not_import_the_legacy_composite_or_callbacks() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "../conduit-form/src/canonical_expansion.rs",
        "../conduit-form/src/canonical_expansion/graph.rs",
        "src/canonical.rs",
    ] {
        let source = fs::read_to_string(manifest.join(relative)).expect("production source reads");
        for forbidden in [
            "conduit_composite",
            "CompositeHost",
            "OperationImplementation",
            "extern \"C\"",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not realize form backs through {forbidden}"
            );
        }
    }
}
