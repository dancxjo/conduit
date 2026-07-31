mod support;

use conduit_panel::Node;
use conduit_runtime::{
    AvailabilityState, Handler, KV_STORE_CONTRACT, PROCESS_SPAWN_CONTRACT, Registry, RunIo,
    RuntimeError, Value, file_read_contract, file_write_contract,
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
fn file_store_and_process_callbacks_cannot_claim_boundary_conformance() {
    for contract in [
        file_read_contract(),
        file_write_contract(),
        &KV_STORE_CONTRACT,
        &PROCESS_SPAWN_CONTRACT,
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
        assert_eq!(availability.rejection_reasons, vec!["CND-RES-025"]);
        let panel =
            conduit_panel::parse(&format!("panel 0\nnode boundary : {}\n", contract.id)).unwrap();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("arbitrary callback is not a compatibility implementation")
                .code,
            "CND-IMP-001"
        );
    }
}
