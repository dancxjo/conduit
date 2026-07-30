use conduit_panel::{Node, parse};
use conduit_runtime::{
    AvailabilityState, DNS_RESOLVER_CONTRACT, Handler, Registry, RunIo, RuntimeError,
    TCP_SOCKET_CONTRACT, UDP_SOCKET_CONTRACT, Value, WIFI_AP_CONTRACT, WIFI_STATION_CONTRACT,
};

struct EchoHandler;
impl Handler for EchoHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let val = inputs.first().cloned().unwrap_or_else(|| Value::text(""));
        Ok(vec![val])
    }
}

#[test]
fn wifi_station_and_ap_nodes_run_in_panel() {
    let panel = parse(
        r#"
            panel 1
            node config_in : conduit/literal { value = "ssid_target" }
            node sta : conduit/wifi-station
            node ap : conduit/wifi-ap
            node sink : conduit/stdout
            cord config_in.out -> sta.in
            cord sta.out -> ap.in
            cord ap.out -> sink.in
        "#,
    )
    .expect("wifi panel parses");

    let registry = Registry::default();

    let avail = registry.node_availability("conduit/wifi-station");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
    assert_eq!(avail.reason_code, "CND-AVL-001");
    assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported wifi nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    let mut custom_registry = Registry::default();
    let reg_provider = |reg: &mut Registry,
                        contract: &'static conduit_core::NodeContract<'static>,
                        factory: conduit_runtime::HandlerFactory| {
        let manifest = conduit_runtime::ImplementationManifest {
            implementation_id: format!("{}.test", contract.id.as_str()),
            contract_id: contract.id.as_str().to_owned(),
            contract_hash: conduit_runtime::compute_contract_hash(contract),
        };
        let host_evidence = conduit_runtime::HostResolutionEvidence {
            host_id: "hosted-local".to_owned(),
            time_basis: "clock/monotonic".to_owned(),
            observed_at_tick: 1,
            valid_until_tick: 1000,
            available_memory_bytes: 1_000_000,
            required_memory_bytes: 1_000,
            rejection_reasons: Vec::new(),
        };
        reg.register_executable_provider(contract, manifest, Some(host_evidence), factory, |_| {
            Ok(())
        })
        .unwrap();
    };

    reg_provider(&mut custom_registry, &WIFI_STATION_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &WIFI_AP_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("wifi panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("wifi panel runs");

    assert_eq!(output, b"ssid_target");
}

#[test]
fn socket_and_dns_nodes_run_in_panel() {
    let panel = parse(
        r#"
            panel 1
            node query : conduit/literal { value = "example.local" }
            node dns : conduit/dns-resolver
            node tcp : conduit/tcp-socket
            node udp : conduit/udp-socket
            node sink : conduit/stdout
            cord query.out -> dns.in
            cord dns.out -> tcp.in
            cord tcp.out -> udp.in
            cord udp.out -> sink.in
        "#,
    )
    .expect("socket panel parses");

    let registry = Registry::default();

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported socket nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    let mut custom_registry = Registry::default();
    let reg_provider = |reg: &mut Registry,
                        contract: &'static conduit_core::NodeContract<'static>,
                        factory: conduit_runtime::HandlerFactory| {
        let manifest = conduit_runtime::ImplementationManifest {
            implementation_id: format!("{}.test", contract.id.as_str()),
            contract_id: contract.id.as_str().to_owned(),
            contract_hash: conduit_runtime::compute_contract_hash(contract),
        };
        let host_evidence = conduit_runtime::HostResolutionEvidence {
            host_id: "hosted-local".to_owned(),
            time_basis: "clock/monotonic".to_owned(),
            observed_at_tick: 1,
            valid_until_tick: 1000,
            available_memory_bytes: 1_000_000,
            required_memory_bytes: 1_000,
            rejection_reasons: Vec::new(),
        };
        reg.register_executable_provider(contract, manifest, Some(host_evidence), factory, |_| {
            Ok(())
        })
        .unwrap();
    };

    reg_provider(&mut custom_registry, &DNS_RESOLVER_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &TCP_SOCKET_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &UDP_SOCKET_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("socket panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("socket panel runs");

    assert_eq!(output, b"example.local");
}
