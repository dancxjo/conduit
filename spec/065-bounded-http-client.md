# Bounded HTTP client boundary

## Contract

`conduit.host/net/http/fetch` performs one explicitly admitted HTTP request and
emits its bounded response. It consumes typed request values and emits typed
response and terminal-result values. HTTP method, authority, target, headers,
body chunks, redirect policy, deadline, and cancellation are semantic inputs;
socket objects, framework clients, browser fetch promises, and provider errors
are not.

The request body and response body are finite ordered byte streams. Header
names and values, counts, aggregate header bytes, body chunks, aggregate body
bytes, redirects, pending operations, connections, retained buffers, timers,
work, evidence, and cleanup all have plan-pinned ceilings. A partial response
reports the committed response head and received bytes before its exact
terminal outcome; it is never silently presented as complete.

## Resolution, authority, and resources

Checking, inspecting, describing, and resolving perform no DNS, socket,
credential, proxy, or HTTP operation. Exact compilation pins:

- provider implementation, artifact, host, and execution profile;
- network resource and fresh topology/provider observations;
- numeric address or explicit resolver result, authority, and destination;
- TLS policy and opaque trust, client-certificate, and private-key handles;
- proxy policy, redirect policy, limits, deadline clock, and timers;
- a scoped outbound grant covering the exact scheme, authority, and resource.

Source, plans, diagnostics, evidence, and Patchbay may retain opaque secret
handle identities but never secret material. An absent outbound client provider
is conforming and reports unsupported.

## HTTP, TLS, redirect, and proxy semantics

HTTP and HTTPS use one request contract, but transport security remains exact.
An HTTPS request requires a TLS-capable provider and the planned trust policy.
Certificate, hostname, trust, client-identity, and handshake failures are
distinct terminal outcomes. HTTPS never redirects or retries as plaintext, and
a redirect from HTTPS to HTTP is rejected before the redirected request.

Redirects are disabled unless explicitly enabled. When enabled, each response,
derived target, authority change, grant check, and remaining redirect budget is
evidence. Loops and limit exhaustion are terminal. Redirect handling never
forwards credentials or restricted headers to a different authority unless an
explicit policy and grant allow it.

A proxy is disabled unless an exact proxy resource is pinned. Environment
variables, operating-system settings, browser policy, and framework defaults
never select a proxy. An unexpected proxy is a binding failure rather than an
ambient fallback.

## Providers

The deterministic provider models request commit, response head and body
chunks, redirect chains, slow or partial responses, cancellation before and
after send, unknown commit, pool exhaustion, provider loss, and bounded
cleanup. Fault controls are observations, not authority.

The Linux hosted provider uses the existing bounded application socket
boundary and the HTTP domain codec. Checked fixtures use explicit loopback
resources and, for HTTPS, fixture trust handles. The provider performs no DNS
and ignores ambient proxy and credential configuration. Browser and constrained
hosts without an exact provider report unsupported.

## Conformance

The checked
[`http-client.json`](../conformance/c4/http-client.json) manifest names every
required success, authority, size, timing, redirect, TLS, proxy, cancellation,
provider-loss, cleanup, hosted-equivalence, and unsupported case. Normalized
evidence preserves HTTP status, body progress, redirect and TLS facts without
retaining credentials, secret material, operating-system handles, or ephemeral
ports.
