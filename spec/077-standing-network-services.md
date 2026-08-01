# Standing network values, services, routing, and observation

Issue: #270. Parent: #133. Provider foundations: #132, #137, #144, #152,
#200-#207, #212, #224.

## Ownership and non-goals

`conduit-net` owns host-neutral network value meanings, finite network state,
and reusable standing link, packet, route, session, service-state, and
observation contracts. `conduit-socket` owns OS TCP/UDP effects;
`conduit-http` owns HTTP, SSE, and WebSocket semantics. NetworkManager,
netlink, kernel packet paths, browser APIs, embedded stacks, deterministic
fixtures, and appliance providers implement compatible subsets; none defines
the graph language.

This specification does not require a new TCP/IP, Wi-Fi, DHCP, DNS, routing,
firewall, or network-management stack. It does not turn library calls,
packets, sessions, questions, answers, or resource records into authored
nodes. A provider may use a capable existing stack behind one exact binding.

## Exact value taxonomy

The current pre-release network package publishes eight base transport and
observation types plus six exact state/observation refinements. The
contracts' descriptors, identities, schemas, bounds, and sensitivities are
exact plan inputs.

| Type | Meaning | Default sensitivity |
| --- | --- | --- |
| `conduit.net/link-observation` | one time-bounded link/interface observation | public |
| `conduit.net/frame` | one bounded link-layer unit | restricted |
| `conduit.net/packet` | one bounded network-layer unit and disposition | restricted |
| `conduit.net/datagram` | one message-preserving transport value | restricted |
| `conduit.net/byte-stream` | ordered bytes with offsets and close facts | restricted |
| `conduit.net/session` | finite conversation identity, generation, and lifecycle | restricted |
| `conduit.net/control-event` | one discrete lifecycle or policy transition | restricted |
| `conduit.net/retained-state` | one finite table/counter snapshot | restricted unless redacted |
| `conduit.net/address-state` | one current address generation and readiness fact | restricted |
| `conduit.net/dhcp-lease` | one finite lease generation and lifecycle value | restricted |
| `conduit.net/neighbor-state` | one finite neighbor entry generation | restricted |
| `conduit.net/route-state` | one finite route-table generation or entry | restricted |
| `conduit.net/service-registration` | one finite named-service registration generation | restricted |
| `conduit.net/reachability-observation` | one scoped, time-bounded non-authority probe result | public or narrowed |

These contracts are not aliases. In particular, a byte stream has no invented
message boundaries; discovery is not a session; a session is not
authentication; authentication is not Conduit membership; and none of those
facts grants an effect.

## Standing port and lifecycle rules

Live inputs and outputs carry zero or more committed values. A live producer
has an open-ended terminal contract; a consumer accepts finite or open-ended
upstream termination. Link observations remain committed observations rather
than retained state. Explicit tables and counters use `LatestState` plus the
finite `RetainedState` temporal boundary.

One provider step performs bounded work. `Produced` keeps the provider live.
When no input, host completion, or timer is ready, the provider registers an
exact interest and returns `Waiting`; it does not spin, sleep, complete, or
advance a clock. Completion, drain, abort, provider loss, and failure are
different outcomes. Editing never starts a run.

The checked deterministic sources use one packet or observation per step,
1,500-byte packets, one exact timer interest, finite one-value current state,
16 routes, eight concurrent sessions, and 64 retained evidence events. Cords
remain independently bounded by their plan-pinned item and byte capacities.
Those fixture values are not universal network limits.

## Current standing panels

- `net/link/observe` publishes fresh link observations and discrete link
  events. The deterministic implementation observes a virtual link and opens
  no interface.
- `net/frame/source` and `net/frame/sink` keep bounded link-layer units,
  interface identity, direction, protocol metadata, observation time, queue
  pressure, and retained counters explicit.
- `net/packet/source` publishes one bounded deterministic packet per exact
  timer wake.
- `net/packet/classify` applies one exact admitted-prefix policy without
  changing topology or authority.
- `net/packet/route` uses a finite longest-prefix route table. Forwarded,
  local-delivery, no-route, hop-exhausted, MTU-exceeded, policy-denied,
  rejected, dropped, and provider-lost dispositions remain distinct.
- `net/packet/sink` consumes packets and publishes a one-value finite counter
  snapshot.
- `net/datagram/source`, `net/datagram/impair`, and `net/datagram/sink`
  preserve message boundaries and make delivered, lost, duplicated, reordered,
  rejected, cancelled, and provider-lost outcomes distinct.
- `net/stream/source` and `net/stream/sink` preserve byte offsets, EOF,
  half-close, and pressure without inventing datagram boundaries.
- `net/session/listen` accepts repeated bounded fixture sessions from one
  long-lived node and publishes session, control-event, and finite session
  table values.
- `net/observe/meter` is an executable meter with explicit input pressure and
  retained counter state.
- `net/observe/service` consumes correlated session, event, and state values
  with zero content retention and finite evidence. A presentation-only Watch
  remains separate and does not add this node to the plan.

`examples/standing-network-packet-path.panel` and
`examples/standing-network-listener.panel` are production-executor proofs.
`examples/standing-network-values.panel` runs frame, datagram, and byte-stream
paths together without flattening them. All enter Waiting, resume on exact
timer wakes, preserve one immutable plan identity, handle repeated runtime
values without topology growth, and end only through explicit lifecycle
control.

The displaced finite text fixtures now have one standing typed form. The
checked isolated-local chain is `net/wifi/access-point` link/address readiness
to `net/dhcp/server` lease state, then `net/dns-sd` service registration and
`net/reachability` scoped observation. Repeated values cross the same four
nodes. No request, lease, name, or probe creates topology.

## Finite route and session state

`RouteTable` holds at most 16 exact entries. Install, replacement, and removal
advance its generation. Selection is deterministic longest-prefix match.
Forwarding decrements the hop limit only after a route exists, policy admits
forwarding, and the payload fits the route MTU. Zero/one-hop packets, absent
routes, denied routes, and MTU failures retain distinct dispositions.

`SessionTable` holds at most eight current sessions. Identity and generation
are separate. A stale-generation transition fails; expiry clears the finite
slot; terminal close, timeout, reset, cancellation, or failure clears the
session without rewriting earlier evidence.

DHCP leases, DNS-SD records, Netherwick registration, and Pico observations
retain the finite generation and expiry rules in
`071-bounded-brainstem-network.md`. A DNS lookup never creates a record.

## Effects, provider facts, and authority

Semantic source may request behavior and finite bounds. It cannot author a
resource, grant, provider observation, initialized interface, authenticated
peer, membership, Internet reachability, or effect authority. The checked
standing fixtures require no authorities because they perform no host network
I/O. That absence is not evidence that a physical provider needs no grant.

Use-time physical effects remain subject to the exact #212 resource/grant
lease and current host observation. Link attachment, address assignment,
DHCP, routes, bridge, forwarding, NAT, firewall, DNS, public listener exposure,
and Internet sharing are independent effects. No panel silently enables a
neighboring effect.

The one current contract-only effect inventory is `net/wifi/join`,
`net/link/wired`, `net/link/virtual`, `net/address/assign`, `net/dhcp/client`,
`net/route/install`, `net/bridge`, `net/forward`, `net/nat`, `net/firewall`,
`net/dns/resolve`, and `net/internet/access`. Each has explicit configuration
or material inputs and exact network outputs. None has a default provider.
Its presence in source therefore authors a request but cannot establish
availability, host state, grant validity, use-time authority, or an effect.
The displaced generic text-shaped `net/*` catalog entries have been removed.

The deterministic router has a second portable userspace implementation
identity to prove provider substitution without ambient route mutation.
Linux socket/HTTP providers remain owned by `conduit-socket` and
`conduit-http`; Pico W overlap remains the bounded AP/DHCP/reachability/DNS-SD
provider contract from #144. Missing physical facts remain contract-only or
unsupported rather than fabricated.

## Tour project

The `library.bounded-brainstem-network` Tour project begins with two isolated
application endpoints and then adds link and address readiness, DHCP, a local
name, an authored userspace route, one repeated-session listener, frame,
packet, datagram and byte-stream exchanges, observation, an invalid loop, and
explicit recovery as separate stages. Every runnable stage uses the production
executor and enters `Waiting`; `examples/standing-network-tour.panel` is the
assembled checkable topology. Keeping the assembled topology check-only makes
the finite scheduler-evidence window visible instead of silently enlarging it.

## Observation

Watch is a bounded presentation subscription to authoritative runtime values.
Attaching or detaching it preserves plan identity, timing, packet delivery,
source cadence, pressure, and routing. A meter, service observer, tee, mirror,
capture, proxy, or relay is an executable graph element and therefore owns
explicit copies, retention, sensitivity, authority, pressure, and failures.
Captured content is never an unbounded permanent log.

## Invalid topology

Cycles require an explicit finite retained-state/delay/lifecycle boundary.
Zero-delay packet recirculation is rejected by compilation rather than broken
by incidental scheduler order. Missing required link/address/route/service
inputs, invalid route prefixes, zero bounds, unsupported provider subsets,
and forbidden source-authored authority remain renderable with exact
diagnostics.

## Conformance inventory

`conformance/c4/standing-network.json` lists the required taxonomy, standing
proofs, provider matrix, state limits, effect separations, and negative
fixtures. The fixture is an executable coverage contract, not evidence that a
host provides every optional network capability.
