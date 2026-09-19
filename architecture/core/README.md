# `conduit-core` responsibility map

`conduit-core` owns the small, `no_std` vocabulary needed to describe Conduit
architecture across unrelated semantic domains. It does not own a value merely
because several hosts use it. Domain packages depend on core; core never
depends on semantic, application, target, or proof packages.

## Retained production modules

| Module | Classification | Core responsibility |
|---|---|---|
| `lib.rs` | universal architecture | Exact identity types and the generic form/host/boot/plan/play/line/sign records shared by the architecture. It is the crate facade, not a domain attic. |
| `base_registry.rs` | universal architecture | Bounded thin-host truth for exact base provider identity, generation, lifecycle, enforcement class, and ordinary capability/resource offer aggregation; no planning, authority issuance, or effects. |
| `base_capability.rs` | universal architecture | Opaque issuer-private base capability possession, exact per-play scope narrowing, finite operation leases, revocation, and non-secret lifecycle inspection. |
| `consequential_effect.rs` | universal architecture | Generic attended last-mile gating for bounded consequential physical effects, including exact resource generation, safety readiness, one-shot authority, uncertain outcomes, and safe disposition. |
| `characteristic.rs` | universal architecture | Generic realization, resource, topology, base, and observation characteristics. |
| `completion.rs` | universal architecture | Exact live-versus-semantic-completion policy sealed from checked form meaning into plan and fragment identity. |
| `configuration.rs` | universal architecture | Generic bounded configuration values carried by checked forms and plans. |
| `control_loop.rs` | universal architecture | Generic plan satisfaction, recovery, and replan decisions over current truth. |
| `execution.rs` | universal architecture | Generic execution-region and admitted execution-profile records. |
| `execution_fusion.rs` | universal architecture | Exact optional fusion of ordinary planned placements without a second executor. |
| `front.rs` | universal architecture | Checked generic capability front and typed port surface. |
| `implementation.rs` | universal architecture | Exact implementation and realization offers, distinct from availability and active instances. |
| `interop.rs` | universal architecture | Exact directional bridge identity, bounded mapping, reflection fencing, and machine-readable refusal without granting sibling authority. |
| `plan_fingerprint.rs` | universal architecture | Canonical fragment and plan commitment encoding; preserves immutable realization identity. |
| `plan_realization.rs` | universal architecture | Exact reusable back identity retained in an expanded form and plan. |
| `port.rs` | universal architecture | Typed port direction and temporal shape. |
| `preparation.rs` | universal architecture | Finite cross-host admission before one exact plan starts. |
| `resource_content.rs`, `resource_canonical.rs`, `resource.rs` | universal architecture | Generic resource requirements, offers, observations, bindings, and compute topology. |
| `resource_admission.rs` | universal architecture | Atomic admission of finite current host resources before play. |
| `route.rs` | universal architecture | Exact planned/admitted line, base, endpoint, authority, and route truth. |
| `shared_pool.rs` | universal architecture | Generic finite shared-pool identity, admission, and placement records. |
| `state_delay.rs` | universal architecture | Explicit typed computational-state identity, continuation, and retained-resource admission. |
| `stream_sampling.rs` | universal architecture | Deterministic bounded source-item selection with exact selected/unselected ownership and accounting, distinct from pressure loss or coalescing. |
| `deadline.rs` | generic mechanism | Exact bounded monotonic-deadline operation/resource contract; no clock implementation or scheduling policy. |
| `delivery.rs` | universal architecture | Versioned delivery/evolution, atomic admission, explicit pressure/coalescing accounting, and finite typed queues without replacing concrete info. |
| `device.rs` | universal architecture | Optional bounded host-observed Device grouping and provenance, validated against exact current host capability truth without granting authority. |
| `resource_reference.rs` | generic mechanism | Portable bounded reference envelope with no path, URL, credential, or ambient authority. |
| `resource_reference_access.rs` | generic mechanism | Separately admitted host-local dereference requirement and outcome. |
| `resource_collection.rs` | generic mechanism | Finite typed collection membership, immutable generations, and bounded deterministic selection. |
| `resource_acquisition.rs` | generic mechanism | Attended resource request, acquisition, release, revocation, loss, and fresh-generation fencing. |
| `info.rs` | generic value mechanism | Minimal bool/scalar envelopes, decode refusal, and semantic digest used by unrelated domains. |
| `quantity.rs` | generic value mechanism | Exact finite dimensioned quantity and exact-only conversion used across unrelated domains. |
| `structured_info.rs` and children | generic value mechanism | Finite canonical structured type/value, selection, inspection, transport, and profile machinery. |
| `temporal.rs` | generic value mechanism | Exact finite temporal identity, instant, relation, and offset-only civil primitives without clocks or timezone databases. |
| `temporal_clock.rs` | generic mechanism | Explicit host/boot monotonic-clock identity and wall-clock correlation truth. |
| `temporal_civil_conversion.rs` | generic value mechanism | Exact offset-only conversion between retained temporal primitives. |
| `temporal_quantity.rs` | generic value mechanism | Exact conversion between generic temporal scales and quantities. |

The temporal primitives above remain core because generic resource references,
observations, plans, and host clock truth require them. Calendar recurrence,
scheduling policy, and calendar-provider semantics live in `semantics/time`.

## Extracted architecture owner

[`conduit-assigned-plan`](../assigned-plan/) owns the finite allocation-free
host-assigned plan schema and validation. `conduit-core` retains a compatibility
re-export of that architecture vocabulary; there is no `assigned_plan.rs` module
in this crate.

## Extracted domain owners

| Domain | Stable owner |
|---|---|
| Artificial life, Lenia, and reaction diffusion | `semantics/alife` |
| Audio values and render demand | `semantics/audio` |
| Calendar, recurrence, and scheduling semantics | `semantics/time` |
| Human interaction, permission-gated media, keyboard events, chords, and keymaps | `semantics/human` |
| JSON value and canonical codec semantics | `semantics/web` |
| Robotics observations and hazard/input values | `semantics/robotics` |
| Patchbay actions and control requests | `products/patchbay/control` |

These moves preserve their existing IDs and encodings. There are deliberately
no compatibility re-exports from `conduit-core`; callers import the owner that
defines the meaning.
