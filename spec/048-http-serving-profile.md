# Composable HTTP serving profile current form

Status: proposed hosted domain and implementation contract

Depends on: specifications 005 through 012, 017, 022, 024, 031, 032, 039,
043, 044, and 046

## Boundary

HTTP is a domain profile above `conduit-core`. Request and response values,
routing, listeners, upgrades, TLS, proxy trust, certificates, sockets, and
framework types do not enter the portable semantic kernel. A handler remains
an ordinary primitive or composite node with typed ports. The reusable
`conduit.http/serve` composite exports request, response, client-event,
view-update, and evidence boundaries; it does not introduce another runtime,
callback graph, route registry, or client store.

The profile keeps these identities distinct:

- HTTP type and node contracts;
- the authored route/composite topology;
- the exact resolved service, implementation, artifact, host report, grant,
  secret scope, listener, security mode, proxy boundary, protocol, and limits;
- live connection, request, and session identities;
- execution and serving evidence;
- HTML/view component meaning, which belongs to the later reusable view
  profile.

## Domain vocabulary

current form defines domain-owned type contracts for request head, bounded body
chunk, response head, header, method, URI, path parameters, exchange identity,
authenticated principal, transport security, client event, server view update,
and protocol failure.

It defines ordinary node contracts for:

- `http-listener`;
- `http-route`;
- `http-response-mux`;
- `http-asset-service`;
- `http-session-gateway`;
- `view-projector`; and
- the exported `serve` composite.

Route matching is method-sensitive and deterministic. The lowest explicit
route order wins, with canonical route identity as the stable secondary order.
Source or registry iteration order is not semantic input. A route exposes
matched path parameters and an explicit unmatched path; response muxing uses
the exact connection/request correlation identity.

Static assets retain their exact artifact identity, media type, grant, path,
and response-byte bound. Asset lookup never reads an ambient filesystem or
turns package presence into authority.

## Exact resolved service

`conduit.resolved-http-service` pins:

- the service contract;
- backend implementation and exact artifact;
- bounded execution profile and resolver-selected placement;
- exact listener endpoint;
- HTTP protocol;
- security descriptor and mode;
- safe certificate identity metadata or an exact trusted-proxy boundary;
- the serving grant and opaque secret scope where direct TLS is selected;
- whether the selected profile requires complete hard enforcement rather than
  observed library/kernel bounds; and
- positive finite head, body, response, header, connection, admission,
  handler, session, session-queue, evidence, timeout, drain, and reserved
  memory limits.

Secret file handles are host-side opaque inputs. Private keys and certificate
bytes never enter panel source, semantic descriptors, exact plan identity,
diagnostics, or evidence. The plan retains safe certificate identity/version
metadata and a secret scope only.

Capability and implementation selection is exact. The resolver checks the
backend, artifact digest, execution profile, endpoint, protocol, security
mode, exact serving grant, host capability limits, and total accounted
adapter/backend/kernel storage. Serving authority is also checked fresh at
bind; an expired or denied grant is not rescued by a valid TLS connection. A
constrained host that cannot satisfy the selected mode returns
`CND-HTTP-007` before listener I/O; capability observation never provisions a
socket, certificate, proxy, DNS record, or firewall.

## Security modes

The only current-form modes are:

1. explicit plaintext;
2. direct TLS using opaque host secret handles; and
3. TLS terminated by one exact trusted proxy peer.

Required TLS never falls back to plaintext. Direct TLS requires a fresh valid
certificate window and certificate identity plus a secret scope. Proxy mode
accepts forwarded scheme, client address, or principal only from the exact
proxy IP and only for individually enabled fields. Forwarded security headers
from any other peer, or in direct/plaintext modes, fail closed. Encryption,
proxy authentication, and Conduit authority remain separate evidence facts.

## Host interface and bounded behavior

`HttpServingBackend` is a poll-based, executor-neutral interface over:

- capability reporting;
- bind using one exact `ResolvedHttpService`;
- accept;
- exchange polling;
- response sending;
- cancellation and closure; and
- bounded structured evidence.

It exposes no Axum, Hyper, Embassy, rustls, socket, async-runtime, or framework
type. The deterministic backend executes admission pressure, parsing, route,
correlation, session, timeout, cancellation, proxy, and shutdown cases. The
Linux reference performs real nonblocking TCP and rustls loopback exchanges
through the same interface. Its buffers and kernel/library resources remain
plan-accounted; a profile that cannot hard-enforce the complete stack may not
claim high assurance.

The implementation rejects malformed request lines and headers, excessive
request or response headers, excessive head/body/response sizes, excessive
connections/admissions/sessions, unsupported upgrades, correlation
mismatches, stale certificate windows, and untrusted forwarded facts with
stable `CND-HTTP-*` reasons. Evidence capacity is reserved and finite; its
exhaustion rejects the next consequential admission before connection state
changes.

WebSocket and server-sent-event upgrades are explicit bounded session modes.
Maximum sessions, queued items, queued bytes, timeouts, cancellation, and
terminal behavior are plan inputs. A slow handler or client reaches a bounded
timeout/cancellation path; it never grows a hidden queue.

Nested views bind an existing domain instance's exported state and intent
ports to a named view projector with finite update size and pending-update
limits. That binding cannot recreate or reinterpret the domain pipeline.

## Serving generations

A replacement never mutates the active service. A candidate with the same
security and protocol floor may prepare a new generation only when combined
old/new reserved memory fits. Commit routes new admissions to the candidate
and lets the old generation drain under its exact deadline. Insufficient
overlap rejects the candidate. A protocol, TLS, proxy, client-authentication,
certificate, or trust-floor downgrade is not graceful degradation.

This contract validates the HTTP-specific drain/rebind floor. The complete
prepare, quiesce, state-transfer, rollback, and retirement transaction remains
owned by issue #57 and must not be inferred from these local checks.

## Constrained-host reference boundary

The adjacent Netherwick `pete-brainstem` Pico W firmware was inspected as an
implementation reference. It uses Embassy/CYW43, fixed TCP receive/transmit
buffers, a fixed request buffer, finite socket and flush timeouts, three HTTP
tasks, and a separate bounded WebSocket task. Those are useful host
implementation shapes, not a Conduit conformance result: the current firmware
owns its route and authority model directly and does not consume an exact
Conduit HTTP service binding or direct-TLS profile.

Accordingly, current form reports the current constrained reference as
deterministically unsupported before start. A future Pico adapter may reuse
the fixed Embassy buffers behind `HttpServingBackend`; it may not copy Pico,
robot, cockpit, or service-specific semantics into `conduit-core`, hide AP
provisioning in HTTP resolution, or claim TLS/HTTP features the firmware does
not implement.

## Stable requirements

- HTTP-001: all HTTP semantics and implementation types remain outside
  `conduit-core`.
- HTTP-002: handlers and `serve` are ordinary typed nodes/composites.
- HTTP-003: route order, fallthrough, path parameters, and response
  correlation are deterministic.
- HTTP-004: request, body, response, connection, handler, session, queue,
  timeout, evidence, and memory bounds are exact plan inputs.
- HTTP-005: resolver selection pins implementation, artifact, profile, host,
  endpoint, protocol, security, grant, and safe certificate/proxy metadata.
- HTTP-006: secret material remains behind opaque host handles.
- HTTP-007: required TLS and proxy trust never downgrade or trust ambient
  forwarded headers.
- HTTP-008: transport authentication and Conduit authority remain distinct.
- HTTP-009: slow, malformed, oversized, cancelled, and exhausted operations
  terminate with stable reasons.
- HTTP-010: static assets retain exact artifact, authority, media, and byte
  bounds.
- HTTP-011: WebSocket and SSE sessions have finite plan-visible resources.
- HTTP-012: real Linux plaintext and TLS implementations use the common
  polling interface.
- HTTP-013: constrained support is selected from actual capabilities and
  rejects unsupported profiles before start.
- HTTP-014: graceful shutdown stops admission and bounds drain/cancellation.
- HTTP-015: serving replacement preserves security floors and accounts both
  generations.
- HTTP-016: complete live-transition semantics remain owned by issue #57.
- HTTP-017: certificate, DNS, firewall, proxy, and network provisioning remain
  external host operations.
- HTTP-018: HTML/view meaning remains owned by issue #90.

The normative fixture is `conformance/c5/http-serving.json`. It contains 49
independently dispatched cases. The Linux plaintext and TLS cases use real
loopback sockets and a generated test trust root. They do not prove public
deployment, certificate provisioning, reverse-proxy installation, DNS,
firewall state, or Internet reachability.
