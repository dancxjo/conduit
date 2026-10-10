# `conduit-core` responsibility map

`conduit-core` owns the small, `no_std` vocabulary needed to describe Conduit
architecture across unrelated semantic domains. It does not own a value merely
because several hosts use it. Domain packages depend on core; core never
depends on semantic, application, target, or proof packages.

## Retained production modules

| Module | Classification | Core responsibility |
|---|---|---|
| `lib.rs` | universal architecture | Exact identity types and the generic plot/host/boot/plan/play/line/sign records shared by the architecture. It is the crate facade, not a domain attic. |
| `base_registry.rs` | universal architecture | Bounded thin-host truth for exact base provider identity, generation, lifecycle, enforcement class, and ordinary capability/resource offer aggregation; no planning, authority issuance, or effects. |
| `base_capability.rs` | universal architecture | Opaque issuer-private base capability possession, exact per-play scope narrowing, finite operation leases, revocation, and non-secret lifecycle inspection. |
| `capability_offer.rs` | universal architecture | Generic bounded host capability offers, realization limits, and exact implementation, operation, resource, and authority requirements. |
| `capability_offer_record.rs` | universal architecture | Exact Host/Boot capability offer record, including finite typed realization properties; semantic domains own their property meanings. |
| `consequential_effect.rs` | universal architecture | Generic attended last-mile gating for bounded consequential physical effects, including exact resource generation, safety readiness, one-shot authority, uncertain outcomes, and safe disposition. |
| `characteristic.rs` | universal architecture | Generic realization, resource, topology, base, and observation characteristics. |
| `completion.rs` | universal architecture | Exact live-versus-semantic-completion policy sealed from checked plot meaning into plan and fragment identity. |
| `configuration.rs` | universal architecture | Generic bounded configuration values carried by checked plots and plans. |
| `flow_semantic_laws.rs` | universal architecture | Finite flow transformation laws and value contracts shared by checking, planning and realization. |
| `control_loop.rs` | universal architecture | Generic plan satisfaction, recovery, and replan decisions over current truth. |
| `execution.rs` | universal architecture | Generic execution-region and admitted execution-profile records. |
| `execution_fusion.rs` | universal architecture | Exact optional fusion of ordinary planned placements without a second executor. |
| `front.rs` | universal architecture | Checked generic capability front and typed port surface. |
| `implementation.rs` | universal architecture | Exact implementation and realization offers, distinct from availability and active instances. |
| `interop.rs` | universal architecture | Exact directional bridge identity, bounded mapping, reflection fencing, and machine-readable refusal without granting sibling authority. |
| `activation_contract.rs` | universal architecture | Exact bounded each, select, and fold coordinator contracts, including finite total item bounds and terminal laws. |
| `plan_fingerprint.rs` | universal architecture | Canonical fragment and plan commitment encoding; preserves immutable realization identity. |
| `plan_realization.rs` | universal architecture | Exact reusable back identity retained in an expanded plot and plan. |
| `planned_activation.rs` | universal architecture | Exact bounded activation of one recursively verified selected Plan, including owner, Value fronts, finite pressure, effect multiplicity, terminal and cancellation laws, and per-activation Sign storage. |
| `planned_gear.rs` | universal architecture | Checked named-field construction and minimum identity validation for one fully selected Gear in a Plan. |
| `planned_gear_record.rs` | universal architecture | One selected Gear record retaining exact implementation, admission, and realization properties in its Plan identity. |
| `realization_properties.rs` | universal architecture | Finite profile-keyed realization property validation, unique profile identities, canonical byte bounds, and generic requirement validation. |
| `port.rs` | universal architecture | Typed port direction and temporal shape. |
| `preparation.rs` | universal architecture | Finite cross-host admission before one exact plan starts. |
| `resource_content.rs`, `resource_canonical.rs`, `resource.rs`, `resource_port.rs` | universal architecture | Generic resource requirements, offers, observations, bindings, compute topology, and the checked distinction between serializable info ports and unforgeable resource-authority ports. |
| `resource_cord.rs` | universal architecture | Exact local Plan Cord transfer of opaque move-only resource possession, including sink acceptance, finite operation use, and lifecycle-bound revocation. |
| `resource_admission.rs` | universal architecture | Atomic admission of finite current host resources before play. |
| `route.rs` | universal architecture | Exact planned/admitted line, base, endpoint, authority, and route truth. |
| `shared_pool.rs` | universal architecture | Generic finite shared-pool identity, admission, and placement records. |
| `state_delay.rs` | universal architecture | Explicit typed computational-state identity, continuation, and retained-resource admission. |
| `source_seeded_state.rs` | universal architecture | Exact declared Source-seeded state boundary validation; initialization remains a dependency and only the declared replacement input is a delay. |
| `stream_sampling.rs` | universal architecture | Deterministic bounded source-item selection with exact selected/unselected ownership and accounting, distinct from pressure loss or coalescing. |
| `terminal_info.rs` | universal architecture | Canonical bounded abnormal-terminal info, including exact category, causal evidence digest, and optional domain-owned fault identity. |
| `deadline.rs` | generic mechanism | Exact bounded monotonic-deadline operation/resource contract; no clock implementation or scheduling policy. |
| `delivery.rs` | universal architecture | Versioned delivery/evolution, atomic admission, explicit pressure/coalescing accounting, and finite typed queues without replacing concrete info. |
| `device.rs` | universal architecture | Optional bounded host-observed Device grouping and provenance, validated against exact current host capability truth without granting authority. |
| `resource_reference.rs` | generic mechanism | Portable bounded reference envelope with no path, URL, credential, or ambient authority. |
| `resource_reference_access.rs` | generic mechanism | Separately admitted host-local dereference requirement and outcome. |
| `resource_collection.rs` | generic mechanism | Finite typed collection membership, immutable generations, and bounded deterministic selection. |
| `resource_acquisition.rs` | generic mechanism | Attended resource request, acquisition, release, revocation, loss, and fresh-generation fencing. |
| `info.rs` | generic value mechanism | Minimal bool/scalar envelopes, decode refusal, and semantic digest used by unrelated domains. |
| `primitive_info.rs` | generic value mechanism | Closed canonical primitive identity registry plus allocation-free validation and count encoding. |
| `value_constraint.rs` | generic value mechanism | Exact finite reusable value contracts, deterministic refusal, and portable bounded text-pattern automata shared by checking, planning, Faces, and Hosts. |
| `fixed_integer.rs` | generic value mechanism | Exact fixed-width signed and unsigned integer values, encoding, and checked arithmetic. |
| `ieee_float.rs` | generic value mechanism | Exact IEEE-754 binary32/binary64 bit identity, canonical little-endian encoding, finite classification, and total ordering without target-native equality drift. |
| `kind_effects.rs` | universal architecture | Reviewed semantic effect facts used to admit pure Kind calls without inferring behavior from names. |
| `retry_evidence.rs` | universal architecture | Finite provider-owned retained evidence for one exact semantic operation, prior realization, and retry law; proves eligibility without inventing a retry loop or treating Step failure as semantic terminal truth. |
| `quantity.rs` and children | generic value mechanism | One canonical bounded decimal Quantity codec carrying its full Unit descriptor, exact literals, physical comparison and rational conversion. |
| `quantity_prefix.rs` | generic value mechanism | Immutable official decimal prefixes and reviewed base-unit composition positions. |
| `quantity_suffix.rs` | generic value mechanism | Whole-suffix resolution with explicit aliases and preserved source spelling, separate from storage eligibility. |
| `unit.rs` and children | generic value mechanism | Catalogue-pinned bounded physical Unit codec, exact scale and affine reference metadata. |
| `quantity_configuration.rs` | generic value mechanism | Checked exact Quantity, physical Unit and temperature-difference startup values with separate bounded source evidence. |
| `structured_info.rs` and children | generic value mechanism | Finite canonical structured type/value, selection, inspection, transport, and profile machinery. |
| `temporal.rs` | generic value mechanism | Exact finite temporal identity, instant, relation, and offset-only civil primitives without clocks or timezone databases. |
| `temporal_clock.rs` | generic mechanism | Explicit host/boot monotonic-clock identity and wall-clock correlation truth. |
| `body_time.rs` | generic mechanism | Bounded Host-local to BodyTime correlation, drift estimation, non-regressing reconciliation, and physical interval relation. |
| `body_time_exchange.rs` | generic mechanism | Four-stamp peer exchange and exact Body, Host, Boot, policy, and generation checks. |
| `body_time_quality.rs` | generic mechanism | Typed uncertainty tolerance and finite-horizon quality admission with execution bounds. |
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
| Patchbay actions and control requests | `plots/patchbay` |

These moves preserve their existing IDs and encodings. There are deliberately
no compatibility re-exports from `conduit-core`; callers import the owner that
defines the meaning.
