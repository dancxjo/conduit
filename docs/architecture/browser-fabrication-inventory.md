# Browser Host fabrication inventory

The authoritative configurable inventory is `BROWSER_IMPLEMENTATIONS` in the browser Host fabrication package. Crèche, BUILD validation, and other configurators consume that package contribution; product code must not maintain another browser implementation list.

Every exposed entry is versioned, targets `browser/wasm32/page`, binds to `conduit.browser/reviewed-distribution@1` / `browser-runtime-superset.wasm`, and carries finite instance and buffered-byte limits. The shared artifact may contain all implementations, but PROFILE admission and current runtime truth remain separate gates.

| Runtime mechanism | Fabrication classification | Runtime prerequisite truth |
| --- | --- | --- |
| DOM presentation | selectable structural and portable presentation Bases | initialized surface |
| keyboard and pointer | selectable portable human-input Bases | focus/page lifecycle; no permission claim at BUILD |
| IndexedDB application storage | selectable durable-storage Base | secure context, availability, quota, schema and corruption checks |
| WebSocket | selectable Line Base | secure context plus endpoint and credential truth |
| WebRTC DataChannel | selectable Line Base | secure context plus negotiated session/grant truth |
| camera and microphone | selectable media Bases | secure context, user activation, permission, device acquisition |
| Web Audio output | selectable audio Base | user activation and current audio context |
| WebSerial and WebUSB | selectable device Bases | secure context, user activation, permission, explicit device acquisition |
| browser Host identity and Body membership | Host mechanism, not a configurable semantic Base | durable profile and admitted membership authority |
| application package loader/presentation bridge | Host operation and product substrate | exact admitted package bytes |
| Book runners and browser proof fixtures | application/proof code, never ordinary Host choices | intentionally excluded |
| touch and gamepad | intentionally unsupported | no reviewed production contract/implementation yet |

Implementation availability, PROFILE selection, Boot initialization, current resource/permission truth, and immutable Plan selection are distinct. BUILD records only the first two. In particular, selecting camera, microphone, WebSerial, or WebUSB never prompts for permission and never claims a device exists.

## Reviewed distribution and ordinary BUILD

The release producer compiles the browser runtime once and seals
`conduit.browser/reviewed-distribution@1`. Its manifest binds the exact runtime
ABI, supported browser target, source commit, producer toolchain, finite file
and bundle sizes, module dependency graph, and every implementation identity
and revision to SHA-256-addressed files.

Ordinary Crèche BUILD is `conduit-host-browser/bind-prebuilt@1`. It does not
invoke Rust, wasm, npm, a registry, a compiler service, or an arbitrary module
loader. It admits that reviewed distribution, resolves every implementation in
the checked PROFILE, and emits `conduit.browser/bundle-image@1` plus the exact
reviewed files. Verification recomputes configuration, PROFILE, distribution,
artifact, implementation, BuildId, IMAGE, and aggregate bundle bindings and
refuses undeclared files.

Two PROFILEs may reuse the same superset `runtime.wasm`; their selected
implementation closures still produce different BuildIds, IMAGE identities,
and BrowserBundle content identities. The bundle remains Body-independent.
Crèche adds `conduit-spore.json` with the Body invitation only afterward, so
HostId, BootId, membership, offers, Plans, and Plays remain runtime truth.

## Profile-gated Boot truth

The reviewed distribution carries a self-contained
`browser-boot-profile.mjs` entry, and the IMAGE binds its exact path and digest.
The Host admits and imports those packaged bytes before instantiating the WASM
runtime. Applications receive no generic module importer or browser-capability
registry: the Host-owned entrance supplies current browser observations to the
exact Boot module and returns only its finite truth projection.

That projection contains entries only for IMAGE-selected implementation
revisions and keeps configured, admitted, initialized, resource-ready, and
offered states separate. Code present in the superset runtime but absent from
the PROFILE is not registered or offered. Unsupported APIs, insecure context,
permission state, user activation, page lifecycle, provider loss, resource
loss, and initialization failure narrow current truth without changing the
PROFILE or IMAGE.

Offer truth is Boot-local and generation-numbered. A lost prerequisite removes
the current offer and yields an exact invalidation for each dependent
realization, retaining its Form and Plan identities while explicitly recording
that neither the authored Form nor IMAGE changed. A reduced-module bundle and
the superset bundle therefore produce the same semantic registry and offers for
one selected PROFILE; only their artifact layout and size may differ.
