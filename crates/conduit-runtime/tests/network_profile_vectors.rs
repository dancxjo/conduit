use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

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
    let resolved = registry.resolve(&panel).expect("wifi panel resolves");

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
    let resolved = registry.resolve(&panel).expect("socket panel resolves");

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
