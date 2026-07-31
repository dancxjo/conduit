# Bounded brainstem network providers

Issue: #144. Parent: #133. Physical equivalence: #142.

## Ownership

`conduit-net` owns reusable Wi-Fi access-point, DHCP-server, ICMP
reachability, and DNS-SD contracts. `conduit-socket` continues to own TCP and
UDP. `conduit-http` continues to own HTTP, SSE, and WebSocket. Netherwick owns
the `motherbrain.pete.internal` composition and all Create UART, stop,
watchdog, reflex, possession, service, safety, and motor authority.

The Pico W firmware is an implementation witness. Its firmware artifact,
build, device, boot, interface, address, provider bundle, resource, grant, and
current observation remain exact binding facts rather than semantic names.

## Finite behavior

The current profile admits eight clients and leases (`192.168.4.2` through
`.9`), eight DNS-SD records, eight port bindings, 1,500-byte packets, 63-byte
names, 64 evidence events, and four ICMP packets per 1,000 monotonic ticks.
Lease, record, registration, cancellation, pressure, malformed input, reboot,
provider loss, and terminal outcomes are explicit and deterministic.

Routing, bridging, and NAT are forbidden. Adding any of them requires a
separately reviewed provider. Discovery is neither enrollment nor identity
proof. Motherbrain registration additionally matches a live DHCP client,
address, lease generation, validated device identity, boot identity, and an
expiry no later than the lease.

## Inventory and observation

A compiled inventory says only which firmware/build and providers are linked.
A describe-only report contains no initialized-provider observation and causes
no radio or socket effect. Runnable admission requires a separately supplied,
fresh current CYW43/AP observation. Loss or expiry changes availability and
terminates an active bounded run; it never rewrites source, contract, plan, or
prior evidence.

The checked deterministic providers are explicitly fixtures and open no radio
or socket. A physical adapter must use the enforced effect backend and must
recheck the exact binding, live provider observation, resource, grant/lease,
and cancellation state at use. A handler may not manufacture initialization,
freshness, permission, possession, service, safety, or motor facts.

## Evidence

`conformance/c4/netherwick-network.json` freezes the ownership map, limits,
negative cases, and no-authority boundary. Host-neutral panels prove the node
contracts. Pico W integration and physical equivalence remain owned by #142.
