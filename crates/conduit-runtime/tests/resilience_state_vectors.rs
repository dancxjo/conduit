mod support;

use conduit_panel::Node;
use conduit_runtime::{
    AvailabilityState, CACHE_CONTRACT, CELL_CONTRACT, CIRCUIT_BREAKER_CONTRACT,
    DEDUPLICATE_CONTRACT, Handler, Registry, RunIo, RuntimeError, Value,
};

struct Impostor;

impl Handler for Impostor {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        Ok(inputs.to_vec())
    }
}

#[test]
fn state_and_resilience_callbacks_cannot_claim_behavioral_conformance() {
    for contract in [
        &CELL_CONTRACT,
        &DEDUPLICATE_CONTRACT,
        &CIRCUIT_BREAKER_CONTRACT,
        &CACHE_CONTRACT,
    ] {
        let mut registry = Registry::default();
        let fixture = support::provider(
            contract,
            &format!("test/{}", contract.id.as_str().replace('/', ".")),
        );
        registry
            .register_executable_provider(
                contract,
                fixture.manifest,
                fixture.artifacts,
                || Box::new(Impostor),
                |_| Ok(()),
            )
            .unwrap();
        let availability = registry.node_availability(contract.id.as_str());
        assert_eq!(availability.state, AvailabilityState::ProviderAvailable);
        assert_eq!(availability.host_id, None);
        let panel =
            conduit_panel::parse(&format!("panel 3\nnode stateful : {}\n", contract.id)).unwrap();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("arbitrary callback is not a compatibility implementation")
                .code,
            "CND-IMP-001"
        );
    }
}
