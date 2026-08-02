use conduit_core::{
    ConnectionCardinality, Delivery, Direction, InterfaceMemberRequirement, LossAcceptance,
    Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract, ValueCardinality,
};
use conduit_runtime::{
    OwnedInterfaceContract, OwnedInterfaceMember, OwnedPrincipalPath,
    OwnedPrincipalProjectionError, OwnedTypeReference,
};

fn member(id: &str, direction: Direction) -> OwnedInterfaceMember {
    OwnedInterfaceMember {
        requirement: InterfaceMemberRequirement::Required,
        id: id.to_owned(),
        direction,
        value_type: OwnedTypeReference {
            id: "fixture/value".to_owned(),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([0x42; 32]),
        },
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        loss: LossAcceptance::LosslessOnly,
    }
}

fn synth_contract() -> OwnedInterfaceContract {
    let mut contract = OwnedInterfaceContract {
        id: "speech/synthesize".to_owned(),
        schema_version: 0,
        principal_path: OwnedPrincipalPath {
            receiving: Some("text".to_owned()),
            outgoing: Some("audio".to_owned()),
        },
        members: vec![
            member("voice", Direction::Input),
            member("audio", Direction::Output),
            member("text", Direction::Input),
        ],
        semantic_hash: SemanticHash::from_bytes([0; 32]),
    };
    contract.semantic_hash = contract.compute_semantic_hash().unwrap();
    contract
}

#[test]
fn hosted_projection_uses_exact_named_principal_members() {
    let contract = synth_contract();
    assert_eq!(
        contract.project_principal(Direction::Input).unwrap().id,
        "text"
    );
    assert_eq!(
        contract.project_principal(Direction::Output).unwrap().id,
        "audio"
    );
}

#[test]
fn member_order_and_auxiliary_ports_cannot_select_a_different_principal() {
    let contract = synth_contract();
    let mut reordered = contract.clone();
    reordered.members.reverse();
    reordered.members.push(member("style", Direction::Input));

    assert_eq!(
        reordered.project_principal(Direction::Input).unwrap().id,
        contract.project_principal(Direction::Input).unwrap().id
    );
    assert_eq!(
        reordered.project_principal(Direction::Output).unwrap().id,
        contract.project_principal(Direction::Output).unwrap().id
    );
}

#[test]
fn absent_principal_never_falls_back_to_a_single_compatible_port() {
    let mut contract = synth_contract();
    contract.principal_path = OwnedPrincipalPath::none();
    contract.members = vec![member("only", Direction::Input)];

    assert_eq!(
        contract.project_principal(Direction::Input),
        Err(OwnedPrincipalProjectionError::Unavailable {
            direction: Direction::Input
        })
    );
}

#[test]
fn principal_path_changes_exact_semantic_identity() {
    let contract = synth_contract();
    let mut changed = contract.clone();
    changed.principal_path.receiving = Some("voice".to_owned());
    changed.semantic_hash = changed.compute_semantic_hash().unwrap();

    assert_ne!(contract.semantic_hash, changed.semantic_hash);
}
