# `conduit-core` responsibility map

`conduit-core` owns the small, `no_std` vocabulary needed to describe Conduit
architecture across unrelated semantic domains. It does not own a value merely
because several Hosts use it. Domain packages depend on core; core never
depends on semantic, application, target, or proof packages.

## Retained production modules

| Module | Classification | Core responsibility |
|---|---|---|
| `lib.rs` | universal architecture | Exact identity types and the generic Form/Host/Boot/Plan/Play/Line/Sign records shared by the architecture. It is the crate facade, not a domain attic. |
| `assigned_plan.rs` | universal architecture | Finite allocation-free validation of one exact Host-assigned Plan projection. |
| `characteristic.rs` | universal architecture | Generic realization, resource, topology, Base, and observation characteristics. |
| `configuration.rs` | universal architecture | Generic bounded configuration values carried by checked Forms and Plans. |
| `control_loop.rs` | universal architecture | Generic Plan satisfaction, recovery, and replan decisions over current truth. |
| `execution.rs` | universal architecture | Generic execution-region and admitted execution-profile records. |
| `execution_fusion.rs` | universal architecture | Exact optional fusion of ordinary planned placements without a second executor. |
| `face.rs` | universal architecture | Checked generic capability Face and typed Port surface. |
| `implementation.rs` | universal architecture | Exact implementation and realization offers, distinct from availability and active instances. |
| `plan_realization.rs` | universal architecture | Exact reusable Back identity retained in an expanded Form and Plan. |
| `port.rs` | universal architecture | Typed Port direction and temporal shape. |
| `preparation.rs` | universal architecture | Finite cross-Host admission before one exact Plan starts. |
| `resource_content.rs`, `resource_canonical.rs`, `resource.rs` | universal architecture | Generic resource requirements, offers, observations, bindings, and compute topology. |
| `resource_admission.rs` | universal architecture | Atomic admission of finite current Host resources before Play. |
| `route.rs` | universal architecture | Exact planned/admitted Line, Base, endpoint, authority, and route truth. |
| `shared_pool.rs` | universal architecture | Generic finite shared-pool identity, admission, and placement records. |
| `state_delay.rs` | universal architecture | Explicit typed computational-state identity, continuation, and retained-resource admission. |
| `deadline.rs` | generic mechanism | Exact bounded monotonic-deadline operation/resource contract; no clock implementation or scheduling policy. |
| `delivery.rs` | universal architecture | Versioned delivery/evolution, atomic admission, explicit pressure/coalescing accounting, and finite typed queues without replacing concrete Info. |
| `device.rs` | universal architecture | Optional bounded Host-observed Device grouping and provenance, validated against exact current Host capability truth without granting authority. |
| `resource_reference.rs` | generic mechanism | Portable bounded reference envelope with no path, URL, credential, or ambient authority. |
| `resource_reference_access.rs` | generic mechanism | Separately admitted Host-local dereference requirement and outcome. |
| `info.rs` | generic value mechanism | Minimal bool/scalar envelopes, decode refusal, and semantic digest used by unrelated domains. |
| `quantity.rs` | generic value mechanism | Exact finite dimensioned quantity and exact-only conversion used across unrelated domains. |
| `structured_info.rs` and children | generic value mechanism | Finite canonical structured type/value, selection, inspection, transport, and profile machinery. |
| `temporal.rs` | generic value mechanism | Exact finite temporal identity, instant, relation, and offset-only civil primitives without clocks or timezone databases. |
| `temporal_clock.rs` | generic mechanism | Explicit Host/Boot monotonic-clock identity and wall-clock correlation truth. |
| `temporal_civil_conversion.rs` | generic value mechanism | Exact offset-only conversion between retained temporal primitives. |
| `temporal_quantity.rs` | generic value mechanism | Exact conversion between generic temporal scales and quantities. |

The temporal primitives above remain core because generic resource references,
observations, Plans, and Host clock truth require them. Calendar recurrence,
scheduling policy, and calendar-provider semantics live in `semantics/time`.

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
