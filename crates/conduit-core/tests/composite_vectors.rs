use conduit_core::{
    CompositeChild, CompositeConfigBinding, CompositeDefinition, CompositeError, CompositeExport,
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, DefinitionDependencies, Delivery, Direction, Id, InstancePath,
    LifecycleState, LossAcceptance, NodeContract, PortContract, PortFlowConstraints, Presence,
    SemanticHash, Sensitivity, TemporalContract, TerminalContract, TypeContractRef,
    ValueCardinality, derive_composite, validate_composite, validate_definition_dependencies,
};

const VALUE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([7; 32]),
};

fn port(id: &'static str, direction: Direction, sensitivity: Sensitivity) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type: VALUE,
        presence: Presence::Required,
        connections: match direction {
            Direction::Input => ConnectionCardinality::ExactlyOne,
            Direction::Output => ConnectionCardinality::OneOrMore,
        },
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

fn field(id: &'static str) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(id),
        value_type: VALUE,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

#[test]
fn exports_retain_every_port_and_config_contract_fact() {
    let child_inputs = [port("child-in", Direction::Input, Sensitivity::Public)];
    let child_outputs = [port("child-out", Direction::Output, Sensitivity::Public)];
    let child_fields = [field("child-setting")];
    let child_contract = NodeContract {
        id: Id("fixture/primitive"),
        config: ConfigContract {
            fields: &child_fields,
        },
        inputs: &child_inputs,
        outputs: &child_outputs,
    };
    let boundary_inputs = [port("in", Direction::Input, Sensitivity::Public)];
    let boundary_outputs = [port("out", Direction::Output, Sensitivity::Public)];
    let boundary_fields = [field("setting")];
    let boundary_contract = NodeContract {
        id: Id("fixture/composite"),
        config: ConfigContract {
            fields: &boundary_fields,
        },
        inputs: &boundary_inputs,
        outputs: &boundary_outputs,
    };
    let children = [CompositeChild {
        id: Id("worker"),
        definition: child_contract.id,
        contract: &child_contract,
    }];
    let exports = [
        CompositeExport {
            boundary_port: 0,
            child: 0,
            child_port: 0,
            direction: Direction::Input,
        },
        CompositeExport {
            boundary_port: 0,
            child: 0,
            child_port: 0,
            direction: Direction::Output,
        },
    ];
    let bindings = [CompositeConfigBinding {
        parameter: 0,
        child: 0,
        child_field: 0,
    }];
    let definition = CompositeDefinition {
        id: boundary_contract.id,
        contract: &boundary_contract,
        children: &children,
        cords: &[],
        exports: &exports,
        bindings: &bindings,
    };
    assert_eq!(validate_composite(&definition), Ok(()));

    let incompatible_outputs = [port("out", Direction::Output, Sensitivity::Secret)];
    let incompatible_contract = NodeContract {
        outputs: &incompatible_outputs,
        ..boundary_contract
    };
    assert_eq!(
        validate_composite(&CompositeDefinition {
            contract: &incompatible_contract,
            ..definition
        }),
        Err(CompositeError::IncompatibleExport)
    );
}

#[test]
fn definition_cycles_and_instance_paths_are_deterministic() {
    assert_eq!(
        InstancePath::new("root/child/attempt.one")
            .unwrap()
            .as_str(),
        "root/child/attempt.one"
    );
    assert_eq!(
        InstancePath::new("root//child"),
        Err(CompositeError::InvalidInstancePath)
    );

    let graph = [
        DefinitionDependencies {
            definition: Id("fixture/a"),
            composite_children: &[Id("fixture/b")],
        },
        DefinitionDependencies {
            definition: Id("fixture/b"),
            composite_children: &[Id("fixture/a")],
        },
    ];
    let mut marks = [0; 2];
    assert_eq!(
        validate_definition_dependencies(&graph, &mut marks),
        Err(CompositeError::RecursiveDefinition)
    );

    let fixture = include_str!("../../../conformance/c2/composite-v1.tsv");
    for case in [
        "one-level",
        "nested",
        "fan-out",
        "parameter",
        "duplicate-export",
        "dangling-export",
        "incompatible-export",
        "recursive-definition",
        "boundary-bypass",
    ] {
        assert!(
            fixture.lines().any(|line| line.starts_with(case)),
            "missing fixture {case}"
        );
    }
}

#[test]
fn flattened_children_derive_the_same_composite_lifecycle() {
    assert_eq!(
        derive_composite(&[LifecycleState::Succeeded, LifecycleState::Succeeded], &[]),
        LifecycleState::Succeeded
    );
    assert_eq!(
        derive_composite(&[LifecycleState::Succeeded, LifecycleState::Failed], &[]),
        LifecycleState::Failed
    );
}
