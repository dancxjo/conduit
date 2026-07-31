use conduit_core::{
    CompatibilityOutcome, ConfigContract, ConnectionCardinality, Delivery, Direction, Id,
    LossAcceptance, NodeContract, PortContract, PortFlowConstraints, Presence, SemanticHash,
    Sensitivity, TemporalContract, TerminalContract, TypeContractRef, ValueCardinality,
    assess_port_substitution, assess_type_contract_exact,
};
use conduit_panel::{
    ContractExportKind, ContractPackageArtifact, ContractPackageExport, ContractPackageLock,
    ContractPackageManifest, LockedContractPackage, LockedExport, parse, resolve_package_imports,
};
use conduit_runtime::{OwnedNodeSchema, Registry};
use sha2::{Digest as _, Sha256};

const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x79, 0xdd, 0x1d, 0x77, 0xe2, 0xcf, 0x64, 0x59, 0xbc, 0x3a, 0x8f, 0x96, 0xc6, 0x5a, 0x91,
        0x5a, 0xdc, 0x10, 0xdb, 0x51, 0x6d, 0xca, 0xc0, 0x39, 0xf7, 0x81, 0xbe, 0xe5, 0xc1, 0xca,
        0xb5, 0xab,
    ]),
};
const FOREIGN_OUTPUT: PortContract<'static> = PortContract {
    id: Id("reading"),
    direction: Direction::Output,
    value_type: VALUE_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::ZeroOrMore,
    values: ValueCardinality::ZeroOrMore,
    delivery: Delivery::Stream,
    temporal: TemporalContract::Progressive,
    terminal: TerminalContract::Either,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
};
const REQUIRED_OUTPUT: PortContract<'static> = PortContract {
    id: Id("value"),
    ..FOREIGN_OUTPUT
};
const CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("example.dev/parts/probe"),
    config: ConfigContract { fields: &[] },
    inputs: &[],
    outputs: &[],
};

fn package(descriptor_hash: String) -> (Vec<u8>, ContractPackageLock) {
    let manifest = ContractPackageManifest {
        schema: "conduit.contract-package".to_owned(),
        draft: 0,
        package_id: "example.dev/parts".to_owned(),
        owner: "Example contract owner".to_owned(),
        provenance: "repository:test".to_owned(),
        license: "MIT".to_owned(),
        dependencies: Vec::new(),
        exports: vec![ContractPackageExport {
            name: "probe".to_owned(),
            canonical_id: CONTRACT.id.to_string(),
            kind: ContractExportKind::Node,
            descriptor_hash: descriptor_hash.clone(),
            descriptor: serde_json::json!({
                "id": CONTRACT.id.as_str(),
                "kind": "node",
                "config": [],
                "inputs": [],
                "outputs": []
            }),
            public: true,
            structural_facets: Vec::new(),
            directional_obligations: Vec::new(),
            conformance_fixtures: Vec::new(),
            lessons: Vec::new(),
            successor: None,
            deprecated: false,
        }],
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let artifact_digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    (
        bytes,
        ContractPackageLock {
            schema: "conduit.contract-package-lock".to_owned(),
            draft: 0,
            packages: vec![LockedContractPackage {
                package_id: manifest.package_id,
                artifact_digest,
                source: "checked test fixture".to_owned(),
                provenance_policy: "repository-owned".to_owned(),
                exports: vec![LockedExport {
                    name: "probe".to_owned(),
                    canonical_id: CONTRACT.id.to_string(),
                    descriptor_hash,
                }],
            }],
        },
    )
}

#[test]
fn import_check_can_succeed_contract_only_without_installing_or_authorizing_a_provider() {
    let source = parse(
        "panel 0\n\
         import example.dev/parts/{probe as inspect}\n\
         node observation : inspect\n",
    )
    .unwrap();
    let descriptor_hash = OwnedNodeSchema::from_contract(&CONTRACT)
        .semantic_hash()
        .to_string();
    let (bytes, lock) = package(descriptor_hash.clone());
    let imports = resolve_package_imports(
        &source,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: Some("local-test-mirror"),
        }],
    )
    .unwrap();

    let mut registry = Registry::default();
    registry.register_contract_only(&CONTRACT);
    let checked = registry.resolve_package_contracts(&imports).unwrap();
    let topology = checked.exact_topology().unwrap();
    assert_eq!(topology.nodes[0].contract_id, CONTRACT.id.as_str());
    assert_eq!(topology.nodes[0].contract_hash.to_string(), descriptor_hash);

    let unavailable = registry
        .resolve(imports.panel())
        .expect_err("an import must not install a provider");
    assert_eq!(unavailable.code, "CND-IMP-001");
    assert!(
        registry
            .installed_providers()
            .iter()
            .all(|provider| { provider.contract.id.as_str() != CONTRACT.id.as_str() })
    );
}

#[test]
fn checker_rejects_a_package_descriptor_that_does_not_match_its_known_contract() {
    let source = parse(
        "panel 0\n\
         import example.dev/parts/{probe}\n\
         node observation : probe\n",
    )
    .unwrap();
    let wrong =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned();
    let (bytes, lock) = package(wrong);
    let imports = resolve_package_imports(
        &source,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap();
    let mut registry = Registry::default();
    registry.register_contract_only(&CONTRACT);
    let failure = registry.resolve_package_contracts(&imports).unwrap_err();
    assert_eq!(failure.code, "CND-IPK-005");
}

#[test]
fn foreign_owned_import_remains_eligible_for_structural_substitution_without_implements() {
    let source = parse(
        "panel 0\n\
         import example.dev/parts/{probe as foreign}\n\
         node observation : foreign\n",
    )
    .unwrap();
    assert!(source.nodes[0].implements.is_empty());
    let descriptor_hash = OwnedNodeSchema::from_contract(&CONTRACT)
        .semantic_hash()
        .to_string();
    let (bytes, lock) = package(descriptor_hash);
    let imports = resolve_package_imports(
        &source,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap();
    let mut registry = Registry::default();
    registry.register_contract_only(&CONTRACT);
    registry.resolve_package_contracts(&imports).unwrap();

    let types = assess_type_contract_exact(REQUIRED_OUTPUT.value_type, FOREIGN_OUTPUT.value_type);
    let substitution = assess_port_substitution(REQUIRED_OUTPUT, FOREIGN_OUTPUT, types);
    assert_eq!(substitution.outcome, CompatibilityOutcome::Compatible);
}
