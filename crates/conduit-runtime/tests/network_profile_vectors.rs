mod support;

use conduit_panel::Node;
use conduit_runtime::{
    AvailabilityState, DNS_RESOLVER_CONTRACT, Handler, Registry, RunIo, RuntimeError, Value,
    WIFI_STATION_CONTRACT,
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
fn wifi_socket_and_dns_callbacks_cannot_claim_network_conformance() {
    let mut contracts = vec![&WIFI_STATION_CONTRACT, &DNS_RESOLVER_CONTRACT];
    contracts.extend(
        [
            "conduit.host/net/tcp/connect",
            "conduit.host/net/tcp/listen",
            "conduit.host/net/udp/connected",
            "conduit.host/net/udp/datagram",
        ]
        .into_iter()
        .map(|id| conduit_std::standard_node_contract(id).unwrap()),
    );
    for contract in contracts {
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
            conduit_panel::parse(&format!("panel 0\nnode network : {}\n", contract.id)).unwrap();
        assert_eq!(
            registry
                .resolve(&panel)
                .expect_err("arbitrary callback is not a compatibility implementation")
                .code,
            "CND-IMP-001"
        );
    }
}
