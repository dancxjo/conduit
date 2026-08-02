# Generic implementation registration and exact binding

Status: candidate normative host/runtime contract.

## One implementation model

An implementation is a host-owned realization of a published semantic node
contract. It is not a second node kind, a source rewrite, an availability
claim, or authority to perform effects. The same registration, observation,
selection, binding, and evidence model applies to media, networking, storage,
learned computation, devices, processes, native libraries, browser code, WASM,
firmware, and remote adapters.

The boundaries are ordered and distinct:

`semantic contract -> installed implementation manifest and artifacts -> host
observation -> conformance and supported subset -> resources and grants ->
exact plan binding -> bounded execution evidence`.

Installing a manifest records only exact static implementation and artifact
facts. It performs no discovery, initialization, provisioning, fetch, network
operation, device open, login, grant acquisition, or media work. Re-registering
an already-known semantic contract is idempotent and must not erase any
installed implementation of that contract.

## Host observation and selection

The host observes its own exact identity, reporter and trust identities, clock,
validity interval, executor, target and ABI support, and finite resource budget.
An implementation observation may additionally name the exact executable,
library, browser-linked code, WASM component, firmware image, or remote adapter
artifact and its supported semantic profile. An installed package name or API
name alone is not a current observation.

Resolution consumes only sealed inputs. It selects among every compatible
implementation for the unchanged contract and may apply a caller-owned ordered
implementation preference. The exact plan pins the chosen implementation,
artifact, host observation, execution profile, allocation, resource bindings,
and grants. Another host or preference produces another plan identity while the
source semantic hash, logical node, ports, types, and cords stay unchanged.

A host may know a contract and install no implementation. That is conforming
and resolves as unsupported. A provider may implement a narrow subset and must
reject an unsupported profile before execution.

## Authority and lifecycle

Installation and observation never grant effects. A process, socket, device,
filesystem, learned-provider, or remote implementation receives independently
observed resource and authority decisions scoped to the semantic instance,
host, run, epoch, clock, validity interval, and cleanup policy. The runtime
checks the exact binding at use.

Finite and standing lifecycles remain semantic contract facts. Implementations
must preserve Waiting, pressure, cancellation, cleanup, partial output, error,
and one terminal cause rather than translating a standing graph into hidden
one-shot work. Process implementations use fixed executable identity, direct
argv, sealed environment, bounded streams and work, and supervised descendant
cleanup.

## Media example

`conduit.media/audio/gain` is one contract. The deterministic Rust reference,
an observed FFmpeg executable, an observed SoX executable, and browser-linked
WASM code may each implement the same exact constant half-gain profile. Their
implementation, artifact, host, limits, process authority, and cleanup facts
differ; the panel does not contain provider names, command lines, or browser
objects.

## Networking analogy

Networking uses this same model, not a parallel provider registry. A semantic
contract such as `conduit.net/packet/route` owns packet, ordering, pressure,
time, lifecycle, and route-result meaning. A portable table, Linux socket or
netlink adapter, browser transport, embedded network stack, or remote carrier
is an implementation with its own exact artifacts and supported subset.

Link availability, interface and address observations, route tables, socket or
radio resources, network effects, grants, carrier limits, and cleanup are host
facts. Merely installing or describing a network implementation cannot open a
socket, associate Wi-Fi, emit a packet, claim a live link, or grant network
authority. Standing network services remain Waiting across traffic and stop
through explicit lifecycle control under the same generic binding.

## Presentation

Patchbay presents the stable logical semantic node first. A separate pinned
realization overlay may show the current implementation, artifact digest, host
observation, supported profile, limits, resources, grants, run state,
cancellation, cleanup, errors, and terminal cause. Presentation never turns
those realization facts into semantic graph species or execution evidence.
