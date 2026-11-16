//! Common finite Fore shape; each operation retains its own exact Kind identity.
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal,
};

pub(crate) fn digest_preparation_kind(
    kind: &str,
    revision: &str,
    request_kind: &str,
    outcome_kind: &str,
) -> Kind {
    let port = |name: &str, kind: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let contract = |name: &str, kind: &str, bound: usize, input| FrontValueContract {
        location: if input {
            FrontValueLocation::Input(port_id(name))
        } else {
            FrontValueLocation::Output(port_id(name))
        },
        contract: CheckedValueContract::new(kind_id(kind), bound as u32, vec![])
            .expect("finite digest preparation value bounds"),
    };
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        inputs: vec![port("request", request_kind, PortDirection::Input)],
        outputs: vec![port("outcome", outcome_kind, PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                contract("request", request_kind, 32, true),
                contract("outcome", outcome_kind, 65, false),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesAfterFixedCount { count: 1 }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (32 + 65) as u32,
        },
    }
}
