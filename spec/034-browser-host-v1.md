# Browser host profile and adapters version 1

Status: candidate normative C5 contract. Browser observations use the generic
fresh `CapabilityReport` model from specification 032 and bind reporter
identity/status through specification 033; they do not create a browser node
kind or change panel semantics.

## Placements and observations

Each report independently describes window, dedicated/shared/service worker,
AudioWorklet, WASM, and WebGPU placements with lifetime, scheduling,
copy/transfer behavior, finite Conduit-owned queue limits, and terminal risks.
API support is distinct from secure context/origin/CSP/Permissions Policy/CORS/
COOP/COEP/isolation facts, permissions, user activation, visibility/focus/
freeze/discard risk, storage quota/persistence/eviction, media/codecs/latency,
graphics/device-loss, and outbound transport facts.

A report pins a realm/entity/passport/status reference for its reporter. Report
collection is an explicit observation operation; resolution only consumes the
frozen report and never feature-detects, enrolls, refreshes, prompts, installs
a service worker, navigates, fetches, connects, listens, or mutates a host.

`granted`, `prompt`, `denied`, and unavailable permission states remain
distinct. `prompt` never satisfies a request. User activation is an ephemeral
report fact; a permission/activation/lifecycle/device-loss change invalidates
the affected report and requires a new exact plan epoch.

## Adapter boundary

The JavaScript adapter implements the ordinary exact-plan protocol through
concrete window, dedicated/shared-worker, already-registered service-worker,
AudioWorklet, WASM, and WebGPU adapters. It admits only a resolved placement,
supplied artifact bytes whose SHA-256 matches the manifest pin, finite message
counts/bytes, and finite provider-response deadlines. Service-worker
registration is a separate explicit effect and is never performed by
resolution or adapter start. The adapters have no DOM/Patchbay dependency.
Worker/worklet death, freeze/discard, page close, storage eviction, network
loss, and GPU loss terminate through structured evidence; external browser or
provider memory is not misreported as Conduit queue capacity. Main-thread and
AudioWorklet work must remain bounded.

Browser endpoints use declared session carriers only. They do not claim raw
TCP/UDP listeners, arbitrary inbound sockets, ambient files/processes/devices,
hard real-time scheduling, or browser-as-server behavior. Carrier choice does
not enter a semantic cord; exact plan, port, grant, delivery, buffer, and
session epoch bindings remain required.

Patchbay presentation/control and browser execution host identities are
separate. Layout, DOM, canvas coordinates, and framework state are
presentation metadata rather than plan/panel identity.

## Required proof profiles

The browser adapter vectors cover realm/passport status binding, stale reports,
prompt-not-granted, activation, bounded queue pressure, exact artifact
integrity, concrete placement execution, and lifecycle/GPU-loss evidence. The
pinned Playwright gate runs the identical vector page in Chromium, Firefox,
and WebKit and records feature-specific unsupported outcomes instead of
branching on browser names. The generic Rust host resolver independently proves
all seven placement modes and a bounded ordinary panel partitioned across a
browser AudioWorklet and Linux remote endpoint. It also resolves one portable
semantic contract independently to browser WASM and a deterministic native
fake while retaining distinct implementation identities. Other reference
profiles cover offline bounded storage, activation-gated audio, and controlled
GPU fallback.
