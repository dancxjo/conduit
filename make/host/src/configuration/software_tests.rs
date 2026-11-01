use crate::test_packages::test_package_set;
use crate::{
    canonical_host_configuration_conduit, check_host_configuration,
    parse_host_configuration_conduit, CheckedHostConfiguration, ImplementationMetadata,
    MakeCatalog, PrerequisiteNode,
};
const SOURCE: &str = r#"host software-fixture {
  schema = 1
  target = {architecture: "x86_64", machine: "computer", os: "linux"}
  base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}
  limits = {static_memory_bytes: 1048576, heap_arena_bytes: 1048576, queue_items: 64, buffered_bytes: 65536, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}
"#;
fn source(software: &str) -> String {
    SOURCE.replace(
        "  limits =",
        &format!("  software = {software}\n  limits ="),
    )
}
fn catalog() -> MakeCatalog {
    let mut catalog = MakeCatalog::canonical().with_packages(&test_package_set());
    for name in ["a", "b"] {
        catalog.implementations.insert(
            format!("fixture/{name}-back@1"),
            ImplementationMetadata {
                kind: format!("fixture/{name}"),
                contract_revision: format!("fixture/{name}@1"),
                targets: vec!["std/*/*".into()],
                prerequisites: vec![],
            },
        );
    }
    catalog
}
fn check(source: &str) -> CheckedHostConfiguration {
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog(),
        &test_package_set(),
    )
    .unwrap()
}
fn selection(name: &str) -> String {
    format!("{{kind: \"fixture/{name}\", contract_revision: \"fixture/{name}@1\", implementation: \"fixture/{name}-back@1\"}}")
}
#[test]
fn omitted_and_explicit_empty_software_have_distinct_identity_and_roundtrip() {
    let legacy = check(SOURCE);
    let empty = check(&source("{capabilities: []}"));
    assert!(legacy.configuration().software.is_none());
    assert!(empty
        .configuration()
        .software
        .as_ref()
        .unwrap()
        .capabilities
        .is_empty());
    assert!(empty.profile().capabilities.is_empty());
    assert_ne!(legacy.configuration_id(), empty.configuration_id());
    assert!(serde_json::to_value(legacy.configuration())
        .unwrap()
        .get("software")
        .is_none());
    for checked in [&legacy, &empty] {
        let rendered = canonical_host_configuration_conduit(checked.configuration()).unwrap();
        let recovered = check(&rendered);
        assert_eq!(recovered.configuration_id(), checked.configuration_id());
        assert_eq!(recovered.configuration(), checked.configuration());
    }
}
#[test]
fn complete_exact_selections_are_checked_and_canonical_order_is_identity_stable() {
    let a = selection("a");
    let b = selection("b");
    let first = check(&source(&format!("{{capabilities: [{a}, {b}]}}")));
    let reversed = check(&source(&format!("{{capabilities: [{b}, {a}]}}")));
    assert_eq!(first.configuration_id(), reversed.configuration_id());
    assert_eq!(
        canonical_host_configuration_conduit(first.configuration()).unwrap(),
        canonical_host_configuration_conduit(reversed.configuration()).unwrap()
    );
    assert_eq!(first.profile().capabilities.len(), 2);
    let selected = &first.profile().capabilities[0];
    assert_eq!(selected.kind, "fixture/a");
    assert_eq!(selected.contract_revision, "fixture/a@1");
    assert_eq!(selected.implementation, "fixture/a-back@1");
    let rendered = canonical_host_configuration_conduit(first.configuration()).unwrap();
    assert_eq!(
        check(&rendered).profile().capabilities,
        first.profile().capabilities
    );
}
#[test]
fn unknown_mismatched_foreign_target_and_unsatisfied_prerequisite_refuse() {
    let selected = source(&format!("{{capabilities: [{}]}}", selection("a")));
    for mutation in ["unknown", "kind", "revision", "target", "prerequisite"] {
        let mut config = parse_host_configuration_conduit(&selected).unwrap();
        let mut catalog = catalog();
        match mutation {
            "unknown" => {
                config.software.as_mut().unwrap().capabilities[0].implementation =
                    "missing/back".into()
            }
            "kind" => {
                config.software.as_mut().unwrap().capabilities[0].kind = "foreign/kind".into()
            }
            "revision" => {
                config.software.as_mut().unwrap().capabilities[0].contract_revision =
                    "foreign/revision".into()
            }
            "target" => {
                catalog
                    .implementations
                    .get_mut("fixture/a-back@1")
                    .unwrap()
                    .targets = vec!["rp2040/*/*".into()]
            }
            _ => {
                catalog
                    .implementations
                    .get_mut("fixture/a-back@1")
                    .unwrap()
                    .prerequisites =
                    vec![PrerequisiteNode::HostCall("fixture/required-call".into())]
            }
        }
        assert!(
            check_host_configuration(config, &catalog, &test_package_set()).is_err(),
            "{mutation} must refuse rather than create readiness"
        );
    }
}
#[test]
fn malformed_duplicate_and_unbounded_purpose_refuse() {
    for invalid in [
        "{}",
        "{capabilities: [], unknown: true}",
        "{capabilities: [ {kind: \"fixture/a\"} ]}",
    ] {
        assert!(parse_host_configuration_conduit(&source(invalid)).is_err());
    }
    let duplicate = source("{capabilities: []}")
        .replace("  limits =", "  software = {capabilities: []}\n  limits =");
    assert!(parse_host_configuration_conduit(&duplicate).is_err());
    let a = selection("a");
    let duplicate = source(&format!("{{capabilities: [{a}, {a}]}}"));
    assert!(check_host_configuration(
        parse_host_configuration_conduit(&duplicate).unwrap(),
        &catalog(),
        &test_package_set()
    )
    .is_err());
    let mut config =
        parse_host_configuration_conduit(&source(&format!("{{capabilities: [{a}]}}"))).unwrap();
    let purpose = config.software.as_mut().unwrap();
    purpose.capabilities = vec![purpose.capabilities[0].clone(); crate::MAX_PROFILE_ITEMS + 1];
    assert!(check_host_configuration(config, &catalog(), &test_package_set()).is_err());
}
