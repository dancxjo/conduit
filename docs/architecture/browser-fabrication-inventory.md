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
| touch and gamepad | portable value contracts and deterministic fixtures exist, but no selectable live browser realization exists | intentionally unadvertised |

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

## Human-facing realization boundary

`BROWSER_HUMAN_PRESENTATION_REALIZATIONS` is the checked join between each
fabrication identity and its portable Kind, ordinary runtime implementation,
runtime artifact identity, Host-operation contract, and finite limits. Runtime
tests compare that table to the real installed `CapabilityOffer`s so a label in
the configurator cannot silently drift away from what the planner selects.

The ordinary installed-browser advertisement is constructed from the selected
fabrication identities. A viewer with only `browser/dom-presentation@1` has no
keyboard offer and no window-input resource. The control-surface profile adds
`browser/keyboard-events@1` and `browser/pointer-events@1`; their presence in a
superset WASM alone grants no offer.

`browser-human-input.mjs` is the shared Host-owned DOM adapter used by browser
products and the pointer vertical. It binds every delivered value to exact
Host, Boot, and offer-generation truth, translates keys and normalized pointer
coordinates into the existing portable value schemas, and keeps unsupported
input, focus/page loss, finite pressure, cancellation, and stale Boot distinct.
The authored Form and portable contract never receive DOM objects, selectors,
CSS, or Web API event classes. Touch and gamepad stay absent until a reviewed
live browser implementation and lifecycle exist; their portable schemas alone
are not an implementation claim.

## Durable-storage realization boundary

`BROWSER_DURABLE_STORAGE_REALIZATION` binds `browser/indexeddb@1` to the
reviewed `browser-application-storage.mjs@1` Host adapter and records the same
finite limits enforced by that adapter: 64 records, 256-byte keys, 64 KiB
values, and 1 MiB per application; at most 16 applications, 1,024 records, and
16 MiB are admitted across one browser Host database. These are Conduit
admission bounds, not a promise that a browser grants or permanently retains
that much storage.

Each admitted application package names its selected Host implementations in
its content digest. The loader initializes application storage only when
`browser/indexeddb@1` is selected and the API is currently available. Superset
JavaScript therefore does not create an offer or an implicit in-memory
substitute. The adapter reports `EvictionPossible`, `PersistenceGranted`, or
`EvictionStatusUnavailable` separately from successful reads and writes.

Application records are namespaced by exact state-compatibility identity and
version. Package content identity remains recorded separately, so compatible
package revisions can intentionally share one state schema. Version mismatch,
corrupt records, application and Host admission exhaustion, browser quota
exhaustion, unavailable storage, explicit deletion, and a stale application
generation are distinct failures or lifecycle states.

The `application-state` and `browser-host-identity` object stores are separate.
Clearing one application's records cannot reset Host identity, mutate Body
membership, or revoke a Host. Book and Crèche receive only the bounded Host
adapter from their application context; neither owns an ambient IndexedDB or
localStorage path.

## Line realization boundary

`BROWSER_LINE_REALIZATIONS` is the checked join from each selectable browser
Line fabrication identity to its exact portable `LineContract`, Base
implementation, reviewed JavaScript artifact, runtime authority requirements,
and finite session, message, queue, and buffer limits. The WebSocket realization
is `conduit.base/websocket-rfc6455@1`: routed-network, message,
full-duplex, ordered, reliable, no continuation, and plaintext-network. The
WebRTC realization is `conduit.base/webrtc-data-channel@1`: point-to-point,
message, full-duplex, ordered, reliable, no continuation, and authenticated and
encrypted. Both admit at most four sessions, one in-flight item, 64 KiB per
application payload, and 256 KiB buffered per session. WebSocket admits a 64 KiB
message; WebRTC separately admits a 128 KiB framed protocol message so its
bounded session control envelope does not consume the application payload.

PROFILE selection alone does not create either offer. Boot must also observe a
current provider, an endpoint grant, and endpoint authority. WebRTC additionally
requires the current Body-scoped signaling bootstrap and session grant. Browser
API presence alone sets none of those authority facts. Addresses, signaling
data, and opaque credentials remain runtime Host inputs and never enter the
authored Form; the browser adapter initiates only explicitly granted outbound
sessions and does not treat network reachability as Body membership.

Provider loss, endpoint-authority loss, signaling loss, session loss, pressure,
cancellation, stale Boot or negotiation identity, unsupported APIs, and finite
capacity exhaustion remain distinct refusals. There is no implicit reconnect:
a later session requires new current authority. The same portable Form and Line
contract may therefore select this browser realization or a materially different
non-browser realization without changing authored transport syntax, because no
such syntax exists in the Form.
