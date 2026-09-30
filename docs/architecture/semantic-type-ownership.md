# Semantic type ownership inventory

**Status:** ownership audit for [#4382](https://github.com/dancxjo/conduit/issues/4382)

**Language owner:** [Conduitese canon #4109](https://github.com/dancxjo/conduit/issues/4109)

**Completed native type and binding foundations:** [#4376](https://github.com/dancxjo/conduit/issues/4376), [#4381](https://github.com/dancxjo/conduit/issues/4381)

This inventory classifies current Rust type *families*. It is not a claim that
the migrations are already complete. Its purpose is to prevent a Rust struct,
enum, or alias from remaining portable semantic truth merely because its
ownership was never examined.

The inventory is exhaustive by source boundary rather than by repeating every
symbol. A row owns every public and private Rust type in its path unless a more
specific row below names an exception. New files inherit their path's class;
moving a type across a boundary requires reviewing its classification.

## Classes

| Class | Meaning | Required disposition |
|---|---|---|
| **P — portable semantic info** | A value whose meaning crosses an exact Fore independent of Host or Rust | Authoritative Conduitese `type`; Rust consumes a generated binding when needed |
| **C — checker/compiler representation** | Parser, checked syntax, semantic-contract model, catalog construction, or lowering machinery | Remains handwritten Rust; may refer to native Type identities but does not become source info |
| **R — runtime identity or evidence** | Host, Boot, Body, Plan, Play, line, sign, admission, reservation, lifecycle, or execution truth | Remains architectural Rust unless an exact Fore deliberately carries a separate portable projection |
| **M — Host/Back/Base mechanism** | Provider state, prepared executor, device driver, adapter, renderer, or platform implementation | Remains handwritten Rust behind the Host boundary |
| **W — external wire/ABI/storage representation** | Bytes owned by a named external protocol, provider schema, firmware ABI, or transport frame | Remains an adapter at that boundary; Conduit-owned compatibility mappings are native `representation` declarations |
| **G — generated binding** | Rust mirror generated from an authoritative native Type | Generated deterministically under #4381; never an independent semantic owner |

Fixtures, test oracles, builders, errors about Rust API misuse, and prepared
allocation objects inherit the class of the machinery they exercise. A typed
failure that actually crosses a Fore is **P**, even when its Rust name ends in
`Error` or `Refusal`.

## Type, representation, binding, and adapter

A semantic `type` states what a value means. A named `representation` states
one portable compatibility contract for realizing that value. Changing a
representation does not change Type identity, and a Type may have several
representations.

A generated binding is target-language machinery derived from checked Types
and representations. It may choose a native target type, a checked wrapper, or
a dynamic carrier, but it never contributes meaning. An external adapter owns
only translation to a separately governed protocol or mechanism. “The mapping
is small” and “Rust already serializes it” do not establish an external
boundary.

The ordinary compact form derives iota tags from authored variant order:

```conduit
type SaveRefusal =
    value_too_large
    | wrong_content_kind

representation data/save-refusal = SaveRefusal as u8
```

The checked representation records `0` and `1`, bounded invalid-tag refusal,
and a compatibility fingerprint. Authors write no tag table or version bump.
When an established contract needs a different order, an indented list states
only that order; the checker still proves it exhaustive and unique.

## Architecture and language implementation

| Scope | Class | Ownership decision |
|---|---|---|
| `architecture/form/**` | C | Lossless/CST syntax, checked forms, expression programs, package resolution, and native-Type checking implement Conduitese. They are not authored info. |
| `architecture/core/src/primitive_info.rs`, `fixed_integer.rs`, `quantity.rs`, `structured_info/**`, `value_constraint.rs` | C | These define the checked value substrate and canonical validation. Primitive and quantity *instances* may be P; the generic representation machinery remains C. |
| Other `architecture/core/**` | R | Exact IDs, Fores, contracts, offers, capabilities, resources, host calls, execution records, and evidence are architectural truth. Portable projections explicitly exposed through a Fore are separate P declarations. |
| `architecture/body/**`, `assigned-plan/**`, `planner/**`, `plan-lowering/**`, `kernel/**`, `observatory/**`, `composite/**` | R | Body, planning, scheduling, admission, and observation state remain architectural runtime truth. |
| `architecture/wire/**`, `protected-line/**` | W | Their frames and session records are explicit transport representations, not portable domain meaning. |

No architecture crate is migrated mechanically. When a semantic crate currently
uses an architecture record as its domain payload, the migration creates a
native P Type and an explicit conversion rather than relabeling the runtime
record.

## Portable semantic domains

The following rows cover every Rust type under `semantics/**`. “P payloads”
means the domain records, enums, finite identifiers, typed terminal payloads,
and constrained scalar wrappers that can appear at a Fore. The other named
families are explicit exceptions.

| Scope | P payloads to migrate | Non-P exceptions and class | Current blocker |
|---|---|---|---|
| `semantics/audio/**` | pitch, note/control events, tone intent, PCM semantic profile/frame/clip, channel layout, gate and terminal meaning | prepared renderers and implementation state are M; codec-only errors are W | remaining payload records and bounded PCM shapes |
| `semantics/chat/**` | chat roles, messages/history, prompt/summary, delivery and presentation state | browser-family installers and state machines are M; fixtures are C | payload-bearing variants, bounded records and terminal review |
| `semantics/data/**` | observations, provenance, measurements, windows, thresholds, plots, tensors, datasets, cadence/continuity, quantity-mapping policy and data-reference domain values | stores/prepared stores and operators are M; wire/codec refusals are W; corpus fixtures are C | remaining payload records, bounded collections and references |
| `semantics/finance/**` | currency, fixed decimal, money, rates | fixtures are C | rates and remaining typed terminal families |
| `semantics/human/**` | input events, modifiers, regions, visual/text/object/motion observations, experience and interaction values | acquisition offers/plans/reservations and active instances are R; initialized implementations are M; codecs are W; conformance vectors are C | bounded strings/collections and payload-bearing variants |
| `semantics/language/**` | typed linguistic terminal payloads | parser/recognizer machinery is M | remaining typed terminal payload review |
| `semantics/net/**` | addresses, endpoints, DNS, attachment info, record-delivery observations and transcript entries | sockets/connections/queues/trackers are M; protocol frames and typed-record codecs are W | payload-bearing variants, records and bounded network values |
| `semantics/presentation/**` | Face subjects, roles, relationships, properties, content, actions, interaction arguments, navigation, composition, temporal facts, graphics commands and Show-visible semantic values | mask plans/admission/lifecycle/sign correlation are R; renderers, queues, ledgers and generators are M; bitmap/graphics encodings are W; migration-era `ApplicationView` scaffolding is C until removed | staged Face-family grammar and migration |
| `semantics/robotics/**` | acceleration, battery, beacon, button, charging, cliff, contact, odometry, orientation, proximity, range and wheel-drop observations | fixture/catalog builders are C | observation records and bounded payloads |
| `semantics/signal/**` | Signal, Trigger and finite pulse/toggle/trigger configurations | encoders are M/W according to the exact carrier | record bindings compatible with fixed no-std carriers |
| `semantics/text/**` | addresses, Morse patterns/segments/transitions and text configuration values | interpreters are M; Kind contracts are C; provider errors remain M unless exported as a typed terminal | bounded text records, payload variants and terminal review |
| `semantics/time/**` | instants, intervals, civil recurrence, calendar/reminder/meeting values, replay commands/results, temporal windows and policies | stores/controllers and `*Back` executors are M; codec forms are W; Kind configuration/checker contracts are C | payload-bearing recurrence, calendar and replay families |
| `semantics/tongues/**` | acoustic/utterance/speech values, recognition results, language evidence and finite research result values when they cross reviewed Fores | recognizers, committers and training machinery are M; dataset/model file representations are W; research harness reports are C unless intentionally exported | Fore-by-Fore ownership review and bounded payload generation |
| `semantics/web/**` | HTTP method/target/header/request/response/body and bounded JSON meaning | server transaction machinery is M; HTTP/JSON byte codecs are W; Kind contracts are C | request/response/body records and JSON payloads |

### Mixed research and catalog crates

These crates contain especially broad mixtures, so their ownership boundary is
stated separately rather than treating the entire crate as portable data.

| Scope | Classification |
|---|---|
| `semantics/ai/**` | Request/result, finite probability, retrieval, grounding, model-description, training-description, relation, citation and typed terminal families that cross Fores are P. Provider sessions, caches, mutable model state, compute offers/runtime identities, vector-index handles/authority, prepared search, lifecycle controllers and host integration are M or R. Provider protocol payloads and model artifact formats are W. Candidate-Form/checker records and fixtures are C. Remaining work is Fore-by-Fore classification plus bounded payload generation; no language or binding prerequisite remains. |
| `semantics/alife/**` | Field/cell/parameter/boundary/partition/work/result values are P. Engines, workers, assemblers and distributed realization state are M. Chunk/line transfer frames are W. Remaining migration depends on bounded-array generation and exact payload review. |
| `semantics/catalog/**` | Catalog installers, `*KindContract`, `*Back`, `Prepared*`, fixtures and conformance helpers are C or M. Domain values currently declared here—navigation goals/poses/routes/trajectories, image/text records, jobs, education/schedule/vision values, garden observations/state, button attempts, palette/pixel regions and typed terminal outcomes—are P and must move to domain-owned `.conduit` source. |
| `semantics/system-continuity/**` | Reboot request/decision/denial and transition causes exposed through reviewed Fores are P. Host instances, assignments, grants, replacement observations, progress state and acceptance receipts are R. Persistence/wire records are W. |

## Mechanisms, products, and targets

| Scope | Class | Ownership decision |
|---|---|---|
| `mechanisms/devices/**`, `mechanisms/implementations/**` | M | Device protocols, prepared implementations, hardware state and provider errors remain behind Base/Back boundaries. Any portable observation they emit uses a native P Type from the relevant semantic domain. |
| `mechanisms/protocols/**` | W | MIDI, Bluetooth and other external protocol records retain their native protocol ownership. Semantic adapters map them explicitly to P Types. |
| `products/**` | R/M | Product models, UI state, CLI records, queues and artifact views are realization/presentation machinery. Domain values shown by a product remain owned by native semantic Types. |
| `targets/browser/**`, `targets/std/**`, `targets/conduitos/**`, `targets/rp2040/**`, `targets/avr/**`, `targets/esp32/**`, `targets/raspberry-pi/**`, `targets/orange-pi/**` | M/W | Host implementations, firmware state, fabrication records, ABI frames and target evidence remain target truth. No target-specific layout enters a portable Type identity. |

## Migration ledger

The inventory above makes the ownership decision; migration receipts record
when the code actually follows it. A family is not complete until all columns
are satisfied.

| Family | Native source | Generated binding | Consumers switched | Duplicate removed | Proof |
|---|---|---|---|---|---|
| Finance currency, comparison, pair, fixed decimal and money | `semantics/finance/types.conduit` | generated at build time | yes | yes | native binding and finance behavior suites |
| Calendar participant role, invitation state and availability state | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus calendar, AI and std Host suites |
| Historical origin/overflow and temporal window boundary/position vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips, Serde compatibility and time behavior suites |
| Timed-pattern refusal vocabulary extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus timed-pattern catalog and std/browser consumer suites |
| Schedule lifecycle and refusal vocabularies extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus schedule catalog and realization suites; schedule window position reuses the existing native temporal-window position instead of retaining a duplicate Rust type |
| Workflow timing outcome and exact duration payloads extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus schedule assessment and realization suites |
| Named pattern-template collection refusal extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus template collection and storage suites |
| Civil recurrence gap, fold and resolution-choice vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips, Serde compatibility and civil recurrence suites |
| Calendar, proposal, recurrence, schedule and temporal-window refusal vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus temporal behavior suites |
| Linguistic offset basis | `semantics/language/types.conduit` | generated at build time | yes | yes | native binding and linguistic suites |
| Audio tone terminal | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact terminal round trip and audio suites |
| Audio gate, modulation destination, PCM sample representation and channel layout | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips plus audio, browser, std and embedded compile suites |
| Sound pressure, cancellation, terminal, stream-state and compatibility-seam vocabularies | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips plus catalog, conformance and Host consumer suites |
| Musical control alternatives and refined payloads | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips and numeric bounds plus Audio, MIDI and synthesizer suites |
| Artificial-life Lenia and reaction-diffusion boundary vocabularies and field-bitmap refusal | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native round trips plus field projection, artificial-life behavior and conformance suites |
| Signal Garden evolution refusal extracted from the catalog | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native round trips plus garden catalog and browser consumer suites |
| AI randomness, draw relationship, probability disposition, log-score and refusal vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus probability, relation, dynamics and training suites |
| Presentation mechanism, status, evidence, choice, navigation, utterance, disclosure and temporal vocabularies | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native round trips plus presentation and consumer suites |
| Robotics beacon/charging, chat connection/presence, HTTP method/failures | domain `types.conduit` sources | generated at build time | yes | yes | exact native round trips plus std/no-std domain suites |
| Robotics simulation availability extracted from the catalog | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native round trips plus robotics catalog and std/ConduitOS consumers |
| HTTP contract refusal vocabulary | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native round trips plus HTTP codec and hosted-HTTP suites |
| HTTP scheme, transaction identity, bounded target and bounded header | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native round trips plus HTTP codec, AI provider, hosted, isolated and ConduitOS consumer suites |
| Body Chat role, bounded message and history record | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact native round trip plus Chat, prompt and std Host suites |
| Chat presentation-state and Body Chat refusal vocabularies | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact native round trips plus presentation, prompt and codec suites |
| Network transport, DNS record/TTL, application refusal, transcript direction and terminal facts | `semantics/net/types.conduit` | generated at build time | yes | yes | exact native round trips plus explicit terminal compatibility codec, network, browser and std Host suites |
| Morse key phase | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trip plus text suites |
| Morse key transition record | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native record round trip plus interpreter and ESP32 tooling suites |
| Address configuration/value and Morse pattern/key refusal vocabularies | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trips plus address and Morse behavior suites |
| Speech commit/recognition dispositions and typed refusals | `semantics/tongues/types.conduit` | generated at build time | yes | yes | exact native round trips plus speech recognition and commit suites |
| Signal, Trigger and finite pulse/trigger/toggle configurations | `semantics/signal/types.conduit` | generated at build time | yes | yes | exact native record round trips plus Signal, std, browser and embedded compile suites |
| Address-detection typed terminal | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trip plus address-detection behavior suite |
| Address-detection result and bounded addressed payload | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trips and index/text bounds plus text, speech and std Host suites |
| Human experience, source-availability, visual-evidence and relation vocabularies | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native round trips plus human experience and visual behavior suites |
| Measurement window/plot policies and threshold state/transitions | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus data and wire suites |
| Measurement window, plot, summary and threshold refusal vocabularies | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus measurement behavior suites |
| Sampled-signal continuity and typed terminal | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips and identity bounds plus sampled-signal behavior suite |
| Data text save/load typed terminals | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus one-byte codec and data behavior suites |
| Tensor element and axis-role vocabularies | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips and bounds plus canonical tensor codec and behavior suites |
| Tensor, scientific-observation, immutable-reference and namespace refusal vocabularies | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus data behavior and codec suites |
| Scientific clock-relation quality | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native payload round trip plus scientific corpus and digest suites |
| Quantity-mapping range, quantization and refusal vocabularies extracted from the catalog | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus catalog, std and browser quantity-mapping suites |
| Scalar comparison vocabulary extracted from the catalog | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus logic catalog and std/browser/ConduitOS consumers |
| Math scalar refusal vocabulary extracted from the catalog | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus math catalog and std/ConduitOS consumers |
| Normalized-quantity refusal vocabulary extracted from the catalog | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus normalized-quantity catalog and browser consumer suites |
| Sequence-normalization and pattern-comparison refusal vocabularies extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus catalog and std/browser consumer suites |
| LLM determinism, terminal-outcome, and implementation-control vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus AI semantic-contract suite |
| AI planning, interruption, candidate and training lifecycle vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus cross-host, composition and training lifecycle suites |
| Reranking strategy and its bounded observed-score payload | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trip plus retrieval and context-selection suites |
| Model operation and port-presence vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus AI model-signature suite |
| Vector similarity and embedding-normalization vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus vector retrieval and canonical serialization suites |
| AI temporal source, boundary, direction, validity, and window vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus temporal context, retrieval, and serialization suites |
| Context selection redundancy, ordering, rationale, and omission vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus context selection and planning suites |
| RAG span, selection, truncation, and grounding vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus RAG semantics and grounded-answer suites |
| Model interpretation and result provenance, disposition, refusal, and failure vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus interpretation and model-result suites |
| House-context and retrieval provenance/proof vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus house-context, hybrid-retrieval, and reranking suites |
| AI result, contract-offer, temporal-context and vector-proof validation vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus AI behavior and contract suites |
| Form Library availability and refusal vocabularies | `semantics/form-library/types.conduit` | generated at build time | yes | yes | exact native round trips and capability-reason bounds plus library behavior suite |
| Body invitation presentation refusal | `semantics/body-invitation/types.conduit` | generated at build time | yes | yes | exact native round trips plus invitation presentation and Body lifecycle suites |
| System-continuity reboot denial | `semantics/system-continuity/types.conduit` | generated at build time | yes | yes | exact native round trip plus continuity and no-std suites |
| System-continuity reboot progress, pending-state and line-loss vocabularies | `semantics/system-continuity/types.conduit` | generated at build time | yes | yes | exact native round trips plus delegated-reboot behavior suite |
| Remaining P families in `semantics/**` | family-owned `.conduit` source required | binding machinery available | in progress | in progress | exact std/browser/ConduitOS/embedded applicability per family |

The completed foundations remove any general “language support” excuse for a
handwritten P declaration. A remaining blocker must now name the exact payload
shape, bound, target constraint or unresolved Fore ownership question. As
migrations land, add reviewed family receipts and keep the classification
history rather than erasing it.

## Enforcement rules

1. A new public domain value under `semantics/**` needs either authoritative
   native Type source or an explicit row here naming its blocker.
2. Generated Rust names, modules and layouts never participate in semantic Type
   identity.
3. `resource T`, capabilities, grants, reservations and active instances are R;
   generation must not make them forgeable serializable P values.
4. Protocol, persistence and ABI types remain W only when their external or
   mechanism contract is named. “Used for serialization” alone is not enough.
5. A handwritten P declaration may disappear only after exact canonical
   encode/decode, bounds/refinements, typed terminal behavior and applicable
   target proof pass against the native source identity.
