# Semantic type ownership inventory

**Status:** completed ownership audit from [#4382](https://github.com/dancxjo/conduit/issues/4382); reviewed migrations continue under [#4375](https://github.com/dancxjo/conduit/issues/4375)

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
| **W — external wire/ABI/storage representation** | Bytes owned by a named external protocol, provider schema, firmware ABI, or transport frame | Remains an adapter at that boundary; Conduit-owned compatibility mappings are native `code` declarations |
| **G — generated binding** | Rust mirror generated from an authoritative native Type | Generated deterministically under #4381; never an independent semantic owner |

Fixtures, test oracles, builders, errors about Rust API misuse, and prepared
allocation objects inherit the class of the machinery they exercise. A typed
failure that actually crosses a Fore is **P**, even when its Rust name ends in
`Error` or `Refusal`.

## Type, code, binding, and adapter

A semantic `type` states what a value means. A named `code` states one portable
compatibility contract for carrying or storing that value. Changing a code
does not change Type identity, and a Type may have several codes.

A generated binding is target-language machinery derived from checked Types
and codes. It may choose a native target type, a checked wrapper, or
a dynamic carrier, but it never contributes meaning. An external adapter owns
only translation to a separately governed protocol or mechanism. “The mapping
is small” and “Rust already serializes it” do not establish an external
boundary.

The ordinary compact form derives iota tags from authored variant order:

```conduit
type SaveRefusal =
    value_too_large
    | wrong_content_kind

code data/save-refusal = SaveRefusal as u8
```

The checked code records `0` and `1`, bounded invalid-tag refusal,
and a compatibility fingerprint. Authors write no tag table or version bump.
When an established contract needs a different order, an indented list states
only that order; the checker still proves it exhaustive and unique. A nonzero
iota origin stays terse and equally checked:

```conduit
code presentation/role = PresentationRole as u8 from 1
```

That records `1`, `2`, and onward in authored variant order. The checker
refuses overflow instead of wrapping or inventing a wider layout.

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
| `semantics/audio/**` | pitch, note/control events, tone intent, PCM semantic profiles/frame headers, channel layout, gate and terminal meaning | prepared renderers and implementation state are M; `PcmClip`, `PcmClipFrame`, `PcmClipError`, and `SoundInfoError` are borrowed codec views or codec/conversion failures and are W | none; the portable family is native and generated |
| `semantics/chat/**` | chat roles, messages/history, prompt/summary, delivery, live-conversation evidence and presentation state | browser-family installers, prompt/state machines and delivery realizations are M; structured-value views and fixtures are C | none; the portable family is native and generated |
| `semantics/data/**` | observations, provenance, measurements, windows, thresholds, plots, tensors, datasets, cadence/continuity, quantity-mapping policy and data-reference domain values | stores/prepared stores, bounded operators and hysteresis/window state are M; `DataReference` is the primitive generation-reference carrier; wire/codec refusals are W; tabular borrowed rows and structured-decoder refusal are W/C | none; the portable family is native and generated |
| Data file-copy terminal result | `semantics/data/types.conduit` | generated at build time; Catalog consumes the generated schema | yes | yes | file-copy contract and hosted copy behavior |
| Reminder occurrence | `semantics/time/types.conduit` | generated at build time; Catalog consumes the generated schema | yes | yes | reminder fixture/delivery Form contracts |
| Normalized pattern comparison result | `semantics/time/types.conduit` | generated at build time; Catalog consumes generated construction/schema | yes | yes | comparison behavior and host contracts |
| Normalized duration and named-pattern template family | `semantics/time/types.conduit` | generated at build time; Catalog retains only normalization/storage behavior | yes | yes | normalization, comparison, bounded collection and storage suites |
| Timed event and interval sequences | `semantics/time/types.conduit` | generated at build time; Catalog retains only interval derivation behavior | yes | yes | ordered-event and pattern-stack suites |
| Schedule effect, lifecycle, position, timing and assessment values | `semantics/time/types.conduit` | generated at build time; Catalog retains scheduling joins and Form registration | yes | yes | schedule and temporal-isomorphism suites |
| `semantics/finance/**` | currency, fixed decimal, money, rates, quotes and transaction events | `FinanceFixture` is C; `FinanceRefusal` is a Rust operation/adapter error joining arithmetic, structured-value and binding failures and is C/W rather than carried Info | none; the portable family is native and generated |
| `semantics/human/**` | input events, modifiers, regions, image/image-text values, experience Fore inputs/projections, and interaction values/outcomes | media offers/plans/reservations/active instances are R; initialized implementations, current-experience stores, interaction flows and keymap state are M; codecs are W; visual assembly views, source adapters, traces and conformance vectors are C | none; the portable family is native and generated |
| `semantics/language/**` | spans, tokens, segments, provenance/evidence, annotations and dependencies | private tokenizer scratch state and parser/recognizer machinery are M; construction/Host errors that never cross a typed Fore are M/C | none; the portable family is native and generated |
| `semantics/net/**` | addresses, endpoints, DNS, attachment info, record-delivery observations and transcript entries | sockets/connections/queues/trackers are M; protocol frames, borrowed codec views, and typed-record codecs are W | none |
| `semantics/presentation/**` | Face subjects, roles, relationships, properties, content, actions, interaction arguments, navigation, composition, temporal facts, graphics commands and Show-visible semantic values | mask plans/admission/lifecycle/sign correlation are R; renderers, queues, ledgers and generators are M; bitmap/graphics encodings are W; migration-era `ApplicationView` scaffolding is C until removed | staged Face-family grammar and migration |
| `semantics/process/**` | executable Job request, argument/environment, output, pressure, usage, lifecycle, exit and terminal-outcome values | planned resource/authority selection and current capability possession are R; OS paths, handles, processes, pipe readers and clocks are M | none; the portable Job family is native and generated |
| `semantics/purpose/**` | finite portable purpose-state and fulfillment-readiness projections | Body purpose, lifecycle and fulfillment objects remain R in `architecture/body/**`; catalog installation is C | none; the portable projection is native and generated |
| `semantics/robotics/**` | acceleration, battery, beacon, button, charging, cliff, contact, odometry, orientation, proximity, range and wheel-drop observations | `RoboticsStructuredFixture` and its structured-value refusal are C; fixture/catalog builders are C | none; the portable family is native and generated |
| `semantics/signal/**` | Signal, Trigger and finite pulse/toggle/trigger configurations | encoders are M/W according to the exact carrier | none; the portable family is native and generated for fixed no-std carriers |
| `semantics/text/**` | addresses, Morse patterns/segments/transitions and text configuration values | interpreters are M; Kind contracts are C; provider errors remain M unless exported as a typed terminal | none; the portable family is native and generated |
| `semantics/time/**` | instants, intervals, civil recurrence, calendar/reminder/meeting values, historical/replay commands and results, temporal windows and policies | stores/controllers and `*Back` executors are M; codec forms are W; Kind configuration/checker contracts are C; `ScheduledIntent<T>` is the generated family’s thin open-generic Rust carrier | none; the portable family is native and generated |
| `semantics/tongues/**` | speech commit/message boundaries, recognition attempts/results, output conditions, terminal outcomes and emitted speech signs | recognizers, committers, acoustic windows and training machinery are M; dataset/model file representations are W; planning/specimen/receipt and research reports are C | none; every reviewed portable Fore payload is native and generated |
| `semantics/web/**` | HTTP method/target/header/request/response/body and bounded JSON meaning | server transaction machinery is M; HTTP/JSON byte codecs are W; Kind contracts are C | recursive JSON values with aggregate depth/node/string budgets |

### Mixed research and catalog crates

These crates contain especially broad mixtures, so their ownership boundary is
stated separately rather than treating the entire crate as portable data.

| Scope | Classification |
|---|---|
| `semantics/ai/**` | Request/result, finite probability, retrieval, grounding, model-description, training-description, relation, citation and typed terminal families that cross Fores are P. Provider sessions, caches, mutable model state, compute offers/runtime identities, vector-index handles/authority, prepared search, lifecycle controllers and host integration are M or R. Provider protocol payloads and model artifact formats are W. Candidate-Form/checker records and fixtures are C. Remaining portable families and their exact blockers are enumerated below; no unclassified AI family remains. |
| `semantics/alife/**` | Field/cell/parameter/boundary/partition/work/result values are P. Engines, workers, assemblers and distributed realization state are M. Chunk/line transfer frames are W. Remaining migration depends on bounded-array generation and exact payload review. |
| `semantics/catalog/**` | Catalog installers, `*KindContract`, `*Back`, `Prepared*`, fixtures and conformance helpers are C or M. Domain values still declared here—image/text records, education/schedule/vision values, garden observations/state, button attempts, palette/pixel regions and typed terminal outcomes—are P and must move to domain-owned `.conduit` source. Job values now belong to Process Conduitese; navigation goals, poses, routes and trajectories belong to Robotics Conduitese. |
| `semantics/system-continuity/**` | Reboot request/decision/denial and transition causes exposed through reviewed Fores are P. Host instances, assignments, grants, replacement observations, progress state and acceptance receipts are R. Persistence/wire records are W. |

### AI ownership completion

Every remaining public AI family has a named owner. Portable families remain
eligible for migration; the blocker column names the missing source construct
or upstream semantic dependency rather than treating handwritten Rust as
authority.

| Remaining family | Class | Exact blocker or retained owner |
|---|---|---|
| Context selection, grounding, interpretation, probability, RAG, relation, reranking, structured-result, temporal and vector request/result records | P | bounded collection fields and fixed digest/byte-array Types must be expressible without weakening their current maxima or exact lengths |
| Generic retrieval and vector families (`StageCandidate<T>`, `RetrievalStage<T>`, `HybridCandidate<T>`, `HybridRetrievalOutcome<T>`, `VectorRecord<T>`, `SimilarityHit<T>`, `VectorSearchValue<T>`) | P | authored generic native Types and generated generic bindings |
| Tensor, sampled-signal, dataset, temporal-zone and scheduled-intent payloads | P with external P dependencies | exact native imports for the owning data/time Types; AI must not counterfeit them locally |
| Float-bearing probability, vector and dynamics values | P | explicit finite/non-finite scalar law and generated equality behavior matching the current semantic contract |
| Boxed relation, training and dynamics outcome trees | P/R boundary | reviewed finite indirection for portable payloads; Host candidates, receipts and realization facts remain R/M |
| Source-extraction receipt proof text | R evidence | current `&'static str` proof-class representation must become a bounded semantic vocabulary before any portable projection |
| Local-model and model-compute offers, sessions and runtime identities | M/R | Host offer, admission and active realization truth; the portable bounded model-cache policy is P and is recorded in the migration ledger |
| Vector-index handles, authorization, mutable state, mutations and maintenance receipts | R | resource authority, generation and execution evidence |
| Cross-host lifecycle, training lifecycle, Host step/candidate/receipt, integration realization and proposal-gate authority families | R/M | Plan, play, authority, active-instance or Host execution truth |
| Form composition candidate and refusal families | C | contain checked/expanded Form compiler representations |
| Provider HTTP/evidence/failure families and explicit codec modules | M/W | provider realization or named byte-protocol ownership |
| Kind/Fore contract descriptor modules and conformance fixtures | C | compiler/catalog declarations and proof fixtures, not carried Info |

## Mechanisms, products, and targets

| Scope | Class | Ownership decision |
|---|---|---|
| `mechanisms/devices/**`, `mechanisms/implementations/**` | M | Device protocols, prepared implementations, hardware state and provider errors remain behind Base/Back boundaries. Any portable observation they emit uses a native P Type from the relevant semantic domain. |
| `mechanisms/protocols/**` | W | MIDI, Bluetooth and other external protocol records retain their native protocol ownership. Semantic adapters map them explicitly to P Types. |
| `products/**` | R/M | Product models, UI state, CLI records, queues and artifact views are realization/presentation machinery. Domain values shown by a product remain owned by native semantic Types. |
| `targets/browser/**`, `targets/std/**`, `targets/conduitos/**`, `targets/rp2040/**`, `targets/avr/**`, `targets/esp32/**`, `targets/raspberry-pi/**`, `targets/orange-pi/**` | M/W | Host implementations, firmware state, make records, ABI frames and target evidence remain target truth. No target-specific layout enters a portable Type identity. |

## Migration ledger

The inventory above makes the ownership decision; migration receipts record
when the code actually follows it. A family is not complete until all columns
are satisfied.

| Family | Native source | Generated binding | Consumers switched | Duplicate removed | Proof |
|---|---|---|---|---|---|
| Finance currency, comparison, pair, fixed decimal and money | `semantics/finance/types.conduit` | generated at build time | yes | yes | native binding and finance behavior suites |
| Finance rate observation and bounded source/profile identities | `semantics/finance/types.conduit` | generated at build time | yes | yes | exact native round trips and text-boundary proof plus finance conversion/reference suites |
| Education question, bounded hints and closed response family extracted from the catalog | `semantics/education/types.conduit` | generated at build time | yes | yes | exact native round trips and three-hint bound plus arithmetic lesson catalog and realization suites |
| Education assessment outcome, assessment, optional hint, evidence/provenance, lesson feedback and progress family extracted from the catalog | `semantics/education/types.conduit` | generated at build time | yes | yes | exact native round trips plus arithmetic and rhythm lesson catalog/realization suites |
| Education rhythm feedback with audio-owned timing feedback | `semantics/education/types.conduit` | generated with an identity-checked external `conduit_audio::TimingFeedback` binding | yes | yes | exact nested-owner/native round trip plus Education and Catalog rhythm-form suites |
| Linguistic span, token, segment, provenance, annotation and dependency family | `semantics/language/types.conduit` | generated at build time | yes | yes | exact four-token/four-annotation hosted proof, bounded text and native round trips |
| Finance observed instant, quote freshness/quote and fixed transaction-event family | `semantics/finance/types.conduit` | generated at build time | yes | yes | exact native identities and round trips, bounded identifiers/sources and finance reference suites |
| Signal Garden state, clock, contact and enriched-observation family | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact bounded native round trips plus deterministic minimal/enriched evolution and authored Form suites |
| Calendar participant role, invitation state and availability state | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus calendar, AI and std Host suites |
| Temporal instant/window, civil and monotonic substrate, calendar, meeting proposal, recurrence and scheduled-intent families | `semantics/time/types.conduit` | generated at build time; the authored generic scheduled-intent family retains its thin Rust generic carrier while concrete payload fields consume generated Types | yes | yes | exact native bounds and round trips, calendar/proposal and recurrence behavior, AI temporal interpretation, presentation conversion and std Host codecs; obsolete struct-literal tests were removed with the handwritten declarations |
| Historical origin/overflow and temporal window boundary/position vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips, Serde compatibility and time behavior suites |
| Pulse observation record | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native record round trip plus canonical six-byte codec and time/browser behavior suites |
| Replay command payload vocabulary | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus preserved six/eight-byte replay command codec and controller suites |
| Calendar candidate-conflict record | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native participant-identity boundaries and round trip plus preserved Serde JSON and proposal behavior |
| Timed-pattern refusal vocabulary extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus timed-pattern catalog and std/browser consumer suites |
| Schedule lifecycle and refusal vocabularies extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus schedule catalog and realization suites; schedule window position reuses the existing native temporal-window position instead of retaining a duplicate Rust type |
| Workflow timing outcome and exact duration payloads extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus schedule assessment and realization suites |
| Named pattern-template collection refusal extracted from the catalog | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus template collection and storage suites |
| Civil recurrence gap, fold and resolution-choice vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips, Serde compatibility and civil recurrence suites |
| Calendar, proposal, recurrence, schedule and temporal-window refusal vocabularies | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus temporal behavior suites |
| Linguistic offset basis | `semantics/language/types.conduit` | generated at build time | yes | yes | native binding and linguistic suites |
| Human image-text metadata record | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native text boundaries and round trip plus preserved image-text digest and codec suites |
| Human image-text record, digest shape and refusal vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native nested-record round trip plus preserved contextual duplicate-key/integrity validation and bounded codec; Catalog and typed-record consumers now use the generated structured conversion directly, with allocation-free nominal canonical validation |
| Human image-observation reference and refusal vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native bounds and refusal round trips plus retained resource-profile, dimension and content-size validation across Human and browser consumers |
| Text address name and bounded address-set family | `semantics/text/types.conduit` | generated at build time | yes | yes | exact 1–8 count and nonempty 64-byte name bounds, native/canonical-value round trips and address-detection suites; Rust retains case-folded uniqueness and punctuation policy |
| Human image-region record | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native U16 boundary round trips, retained image-relative validation and unchanged vision structured codec suites |
| Audio tone terminal | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact terminal round trip and audio suites |
| Data measurement plot point | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native scalar bounds and round trip plus preserved bounded-series wire codec and projection proof |
| Data clock relation bounded record | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native text and positive-scale boundaries plus preserved scientific digest and alignment suites |
| Audio gate, modulation destination, PCM sample representation and channel layout | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips plus audio, browser, std and embedded compile suites |
| Sound pressure, cancellation, terminal, stream-state and compatibility-seam vocabularies | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips plus catalog, conformance and Host consumer suites |
| Musical control alternatives and refined payloads | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trips and numeric bounds plus Audio, MIDI and synthesizer suites |
| Musical control event bounded record | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native time bounds, preserved 22-byte codec and digest golden plus MIDI and synthesizer consumers |
| Musical pitch bounded record | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trip and numeric bounds, preserved authored constructor order, const value access and 20-byte codec golden plus MIDI and synthesizer consumers |
| Musical note occurrence identity and event family | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native identity/event round trips and bounds, preserved authored constructor order, 43-byte codec and digest golden plus MIDI, synthesizer, std and ConduitOS consumers |
| Tone intent bounded record | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trip and numeric bounds, preserved authored constructor order, 41-byte codec and digest golden plus ConduitOS speaker consumers |
| PCM clip profile bounded record | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact native round trip and sample-rate/clock boundaries plus canonical PCM clip codec and std speech consumers |
| Artificial-life Lenia and reaction-diffusion parameter/boundary vocabularies, region identity and field-bitmap refusal | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact parameter bounds/native round trips, preserved owner validation and field/request codecs/digests, full region-identity scalar domain and preserved two-byte boundary-wire placement plus field projection, artificial-life behavior and conformance suites |
| Artificial-life reaction-diffusion region geometry and bounded partition | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native scalar/sequence round trips plus owner-mapped count refusal and preserved duplicate, overlap, gap, boundary and two-host region-work proofs; boundary/state containers retain handwritten cross-record canonicality |
| Signal Garden evolution refusal extracted from the catalog | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native round trips plus garden catalog and browser consumer suites |
| AI randomness, draw relationship, probability disposition, log-score and refusal vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus probability, relation, dynamics and training suites |
| AI source-extraction profile and overlap payloads | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus Serde/postcard compatibility and source-extraction suites |
| AI finite classification and validated extraction families | `semantics/ai/types.conduit` | generated at build time with the established JSON record boundary | yes | yes | exact nonempty 64-byte label/key, 1,024-byte value, and 32-member bounds; native round trips; retained label-membership and duplicate-key validation; hosted local-model consumers |
| AI confidence and model-work accounting values | `semantics/ai/types.conduit` | generated at build time with retained Copy/Serde bindings | yes | yes | exact 0–1,000 confidence boundary, full U64 accounting fields and native round trips plus LLM contract, provider and Patchbay consumers; impossible over-range confidence tests removed |
| AI reranking policy | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact nonempty 256-byte identity, 1–1,024 candidate and 1–1,048,576 work-unit bounds plus native round trip and context-selection/RAG behavior; scorer observations and receipts remain in the connected generic retrieval migration |
| AI context-selection policy | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact nonempty 256-byte policy/accounting identities and intrinsic item, byte, token and work bounds plus native round trip, visible truncation and six-question RAG behavior; candidate/result records remain in the connected generic retrieval migration |
| AI grounded-answer policy | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact nonempty 256-byte policy/answer-kind identities and intrinsic positive output, claim, citation and work bounds plus native round trip, grounding, RAG and Pete recollection consumers; request/claim/result records remain in the connected generic retrieval migration |
| AI profile-reported confidence | `semantics/ai/types.conduit` | generated at build time with retained Copy/Serde bindings | yes | yes | exact 0–1,000 score boundary and native round trip plus interpretation and hosted local-model consumers; deliberately not probability |
| AI generated-text chunk | `semantics/ai/types.conduit` | generated at build time with retained Serde binding | yes | yes | exact 0–4,095 sequence and nonempty 4,096-byte text bounds, native round trip and literal canonical-codec golden plus streaming, model-text, std Host and live-conversation consumers |
| AI generated-text flow evidence | `semantics/ai/types.conduit` | generated at build time with retained Copy/Serde binding | yes | yes | exact 0–4,096 chunk and 0–65,536 byte bounds, zero-count/zero-byte coherence and native round trips plus Chat projection, Patchbay presentation and std Host streaming consumers |
| AI clock basis and temporal reference | `semantics/ai/types.conduit` | generated at build time with retained Serde variant order | yes | yes | exact nonempty 128-byte monotonic identity bound, full U64 reference instant and native round trips plus temporal, interpretation, retrieval and Pete consumers; provenance relations remain behavioral Rust |
| AI hybrid-retrieval fusion strategy and bounded rank payload | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trips, invalid-boundary refusal and hybrid retrieval/RAG suites |
| AI retriever identity | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact nonempty 256-byte identity bound and mechanism composition plus native round trip, hybrid retrieval codec/kernel, RAG, context selection and Pete consumers |
| AI relation result profile and positive sample-bound payload | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trips, invalid-boundary refusal, preserved manual digest bytes and relation/Tongues suites |
| AI model-cache policy and positive model/byte bounds | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trips, invalid-boundary refusal and model-compute/Tongues/std Host suites; offer-relative byte adequacy remains Rust validation over portable policy and Host offer truth |
| Presentation mechanism, status, evidence, choice, navigation, utterance, disclosure and temporal vocabularies | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native round trips plus presentation and consumer suites |
| Presentation layout axis and alignment vocabularies | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native round trips plus layout and renderer consumer suites |
| Presentation graphics clip-class vocabulary | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exhaustive native round trips plus graphics classification, Patchbay and ConduitOS consumer suites |
| Presentation graphics point record | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact signed-coordinate native boundary round trips plus preserved literal orthogonal-path codec and Patchbay/ConduitOS consumers |
| Face utterance provenance and bounded identity payloads | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native bounds and round trips, legacy digest goldens, aural projection and Body/Patchbay consumers |
| Generated manifestation disposition and content-role vocabularies | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exhaustive native round trips plus exact Serde/postcard compatibility and generative manifestation consumer suites |
| Generated action-affordance bounded record | `semantics/presentation/types.conduit` | generated at build time with exact deny-unknown Serde binding | yes | yes | native identity bounds, exact JSON/postcard and digest compatibility, generative validation and std presenter consumers |
| Presentation theme RGB color record | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native RGB round trip, preserved const access and three-byte application-theme wire placement plus Patchbay theme suites |
| Robotics beacon/charging, chat connection/presence, HTTP method/failures | domain `types.conduit` sources | generated at build time | yes | yes | exact native round trips plus std/no-std domain suites |
| Robotics body-frame acceleration observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native and axis-boundary proof plus preserved 12-byte codec, digest golden and decode refusals |
| Robotics cliff observation and unavailable/observed signal | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus preserved cliff codec, digest golden and behavior suites |
| Robotics body-sector proximity observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native mask bounds plus preserved one-byte codec, digest golden and typed decode refusals |
| Robotics body-sector contact observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native mask bounds plus preserved public projection, one-byte codec, digest golden and typed decode refusals |
| Robotics wheel-drop observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native wheel-mask bounds plus preserved public projection, one-byte codec, digest golden and typed decode refusals |
| Robotics forward range observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native and field-boundary proof plus preserved constructor order, 8-byte codec, digest golden and decode refusals |
| Robotics battery observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native and field-boundary proof plus preserved quantity projections, 4-byte codec, digest golden and decode refusals |
| Robotics body-frame orientation observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native axis bounds plus preserved components, 12-byte codec, digest golden and decode refusals |
| Robotics start-local odometry observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native axis/yaw bounds and round trip plus preserved component API, 12-byte codec, digest golden and typed decode refusals |
| Robotics button-set observation | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native U32 round trip plus preserved public projection, four-byte codec, digest golden and typed decode refusal |
| Robotics simulation availability extracted from the catalog | `semantics/robotics/types.conduit` | generated at build time | yes | yes | exact native round trips plus robotics catalog and std/ConduitOS consumers |
| Robotics navigation goal, pose, grid, route, trajectory, planning and local-control family extracted from the catalog | `semantics/robotics/types.conduit` | generated at build time | yes | yes | catalog ports and codecs consume the native family directly; exact native bounds and round trips, signed-origin/unsigned-extent law, maximum identity/sequence proof, and preserved route/control behavior plus consumer Form/planner proof |
| HTTP contract refusal vocabulary | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native round trips plus HTTP codec and hosted-HTTP suites |
| HTTP scheme, transaction identity, bounded target and bounded header | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native round trips plus HTTP codec, AI provider, hosted, isolated and ConduitOS consumer suites |
| Body Chat role, bounded message and history record | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact native round trip plus Chat, prompt and std Host suites |
| Body conversational summary record | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact native and JSON round trips, retained canonical field order, text bounds and Body Chat digest suites |
| Chat presentation-state and Body Chat refusal vocabularies | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact native round trips plus presentation, prompt and codec suites |
| Network transport, DNS record/TTL, application refusal, transcript direction and terminal facts | `semantics/net/types.conduit` | generated at build time | yes | yes | exact native round trips plus explicit terminal compatibility codec, network, browser and std Host suites |
| Bounded DNS query record | `semantics/net/types.conduit` | generated at build time | yes | yes | exact native record round trip and name/port bounds plus network catalog and std resolver suites |
| Bounded network attachment identity | `semantics/net/types.conduit` | generated at build time with retained string Serde adapter | yes | yes | exact native text round trip, 96/97-byte boundary, JSON compatibility and existing attachment-wire suites |
| Morse key phase | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trip plus text suites |
| Morse segment record | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native record round trip plus canonical Morse codec and interpreter suites |
| Morse key transition record | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native record round trip plus interpreter and ESP32 tooling suites |
| Address configuration/value and Morse pattern/key refusal vocabularies | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trips plus address and Morse behavior suites |
| Speech commit/recognition dispositions and typed refusals | `semantics/tongues/types.conduit` | generated at build time | yes | yes | exact native round trips plus speech recognition and commit suites |
| Live-conversation speech capacity requirements | `semantics/tongues/types.conduit` | generated at build time | yes | yes | exact native full-domain round trip plus portable capacity and std Host consumer suites |
| Signal, Trigger and finite pulse/trigger/toggle configurations | `semantics/signal/types.conduit` | generated at build time | yes | yes | exact native record round trips plus Signal, std, browser and embedded compile suites |
| Address-detection typed terminal | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trip plus address-detection behavior suite |
| Address-detection result and bounded addressed payload | `semantics/text/types.conduit` | generated at build time | yes | yes | exact native round trips and index/text bounds plus text, speech and std Host suites |
| Human experience, source-availability, visual-evidence and relation vocabularies | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native round trips plus human experience and visual behavior suites |
| Human experiencer source inputs, source status, and current projection | `semantics/human/types.conduit` | generated at build time; Catalog installs the generated nominal Types directly | yes | yes | nominal-distinction, finite-projection, and portable experiencer suites |
| Human experience temporal policy and refusal | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native U64 round trip with authored current-before-recent refinement plus experience classification suites |
| Human keymap refusal vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native round trips plus keymap and std/browser/ConduitOS consumer suites |
| Human key-modifier bitset scalar | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native full-U8 round trips plus preserved const masks, key-event/chord codecs and semantic digests |
| Human camera/microphone media-kind vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native and stable JSON/postcard round trips plus human-media planning and target consumer suites |
| Human visual-impression disposition vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native round trips, invalid-truncation validation, catalog codec/digest and std hosted-vision suites |
| Human visual observation, impression and aggregate-experience refusal families | `semantics/human/types.conduit` | generated at build time | yes | yes | exhaustive native payload round trips plus observation, impression, aggregate-experience and source-adapter behavior suites; Rust retains relational validation but no duplicate refusal vocabulary |
| Human current-experience admission, source-adaptation, revision-update and inspection refusal families | `semantics/human/types.conduit` | generated at build time | yes | yes | exhaustive native terminal and nested-payload round trips plus current-experience admission, source adaptation, transactional update/history and exact-item inspection suites; Rust retains bounded state transitions but no duplicate refusal vocabulary |
| Human payload-rich interaction-family contract | `semantics/human/types.conduit` | generated at build time; thin ergonomic constructors retain Kind identity inputs for Rust callers | yes | yes | all eight family shapes, exact 128-byte value-kind and 32-byte type-digest boundaries, native round trips, canonical contract identities, validation, bounded flow and browser source-interaction compilation |
| Human interaction value and proposal-payload family | `semantics/human/types.conduit` | generated at build time; thin adapters retain Kind identity and ordinary Vec inputs at Rust construction seams | yes | yes | exact nonempty 128-byte value-kind, 65,536-byte canonical payload and 64-value selection bounds; activate, absolute-values and relative-value native round trips; preserved canonical identities, validation, bounded flow, browser source interaction and Patchbay configuration consumers |
| Measurement window/plot policies and threshold state/transitions | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips plus data and wire suites |
| Measurement threshold policy record | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native full-domain Quantity round trip, retained unit/order validation and literal hysteresis-profile wire golden |
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
| Training objective participation, batch order, checkpoint/evaluation policies, model-compute operation/lifecycle/class/refusal, model signature/evidence/compatibility/text refusals, and vector-index health/maintenance vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus training, model artifact/text/compute/signature and vector-index lifecycle suites |
| Local-model profile, cache, lifecycle, refusal, failure and offer-validity vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus local-model offer and hosted-adapter suites |
| Local-model and model-invocation payload-bearing terminals | `semantics/ai/types.conduit` | generated at build time with direct Type payloads | yes | yes | exact native payload round trips, Serde generation and model lifecycle suites |
| Interpretation invalidity and temporal interpretation/evidence-selection refusals | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus interpretation and temporal-context suites |
| Context-selection, reranking and wired-house-context refusals | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus selection, reranking and house-context suites |
| Generated-text-flow and continuous-dynamics refusals | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus streaming-generation and dynamics suites |
| Relation, training-step and integration terminal failures plus embodiment stages | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus relation, training, dynamics and embodiment suites |
| Continuous-dynamics integration accuracy record | `semantics/ai/types.conduit` | generated at build time with retained public fields | yes | yes | exact authored nonzero tolerance/error laws, native boundary round trips and dynamics refusal/behavior suites |
| Relation query/refusal, training refusal and vector/index-resource refusal vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus relation, training and vector-resource behavior suites |
| Exact-vector-search payload-bearing refusal | `semantics/ai/types.conduit` | generated at build time with direct Type payloads | yes | yes | exact native round trips plus bounded exact-search behavior suite |
| AI planning, interruption, candidate and training lifecycle vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus cross-host, composition and training lifecycle suites |
| Reranking strategy and its bounded observed-score payload | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native payload round trip plus retrieval and context-selection suites |
| Model operation and port-presence vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus AI model-signature suite |
| Missing-modality training policy and bounded declared modalities | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native bounds and round trips, retained relational validation, exact legacy training digest, and training lifecycle suites |
| Positive fixed and bounded model-dimension constraints | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native bounds, retained range/product validation, legacy digest goldens, and model-signature consumers |
| Vector similarity and embedding-normalization vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus vector retrieval and canonical serialization suites |
| Finite compatible-vector-metrics record | `semantics/ai/types.conduit` | generated at build time with retained Rust Copy/Serde binding traits | yes | yes | exhaustive Boolean combinations, exact JSON/postcard compatibility, and vector/RAG/std consumer suites |
| AI temporal source, boundary, direction, validity, and window vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus temporal context, retrieval, and serialization suites |
| Context selection redundancy, ordering, rationale, and omission vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus context selection and planning suites |
| RAG span, selection, truncation, and grounding vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus RAG semantics and grounded-answer suites |
| Model interpretation and result provenance, disposition, refusal, and failure vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus interpretation and model-result suites |
| Model-result disposition and grounded-answer payload-bearing refusal | `semantics/ai/types.conduit` | generated at build time with direct Type payloads | yes | yes | exact native round trips plus model-result and grounded-answer suites |
| House-context and retrieval provenance/proof vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus house-context, hybrid-retrieval, and reranking suites |
| Hybrid-retrieval mechanism and reranking score payloads | `semantics/ai/types.conduit` | generated at build time with direct scalar payloads | yes | yes | exact native round trips plus hybrid-retrieval and reranking suites |
| AI result, contract-offer, temporal-context and vector-proof validation vocabularies | `semantics/ai/types.conduit` | generated at build time | yes | yes | exact native round trips plus AI behavior and contract suites |
| Form Library availability and refusal vocabularies | `semantics/form-library/types.conduit` | generated at build time | yes | yes | exact native round trips and capability-reason bounds plus library behavior suite |
| Body invitation presentation refusal | `semantics/body-invitation/types.conduit` | generated at build time | yes | yes | exact native round trips plus invitation presentation and Body lifecycle suites |
| Tutorial playback phase vocabulary | `semantics/tutorial/types.conduit` | generated at build time | yes | yes | exact native and Serde round trips plus Tutorial behavior suites |
| Home destination vocabulary | `semantics/home/types.conduit` | generated at build time | yes | yes | exact native round trips plus finite launcher navigation and Home behavior suites |
| System-continuity reboot denial | `semantics/system-continuity/types.conduit` | generated at build time | yes | yes | exact native round trip plus continuity and no-std suites |
| System-continuity reboot progress, pending-state and line-loss vocabularies | `semantics/system-continuity/types.conduit` | generated at build time | yes | yes | exact native round trips plus delegated-reboot behavior suite |
| HTTP exchange target, headers, body and request/response family | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native bounds and round trips plus HTTP, provider, hosted and ConduitOS consumer suites |
| JSON parse, collection and summary refusal families | `semantics/web/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus preserved detail-code adapters and std/browser consumer proof |
| Replay terminal state and civil-resolution policy | `semantics/time/types.conduit` | generated at build time | yes | yes | exact native round trips plus replay, recurrence and std Host consumer suites |
| Rhythm state and synchronization outcome | `semantics/time/types.conduit` | generated at build time | yes | yes | exact full-domain native round trips plus retained 14-byte codec, synchronization behavior and browser consumer proof |
| PCM compatibility profile and incompatibility-reason family | `semantics/audio/types.conduit` | generated at build time | yes | yes | exact primitive-domain native round trips plus Audio/Catalog compatibility behavior and Serde proof |
| Portable keyboard occurrence | `semantics/human/types.conduit` | generated at build time | yes | yes | exact usage/refinement proof plus keymap and target consumer suites |
| Portable keymap Unicode-scalar fragment and output disposition | `semantics/human/types.conduit` | generated at build time as Copy, allocation-free bindings | yes | yes | exact scalar-domain boundary and native payload round trips plus compose/Unicode behavior and std/browser/ConduitOS/Patchbay consumer compilation; UTF-8 is encoded into caller-owned four-byte storage at the target boundary |
| Portable modifier-chord meaning and finite control-modifier payload | `semantics/human/types.conduit` | generated at build time as Copy, allocation-free bindings | yes | yes | all eight native alternatives and three control-modifier payloads round-trip through the retained exact four-byte codec and semantic digest; focused input behavior plus std/browser/ConduitOS/Patchbay consumer compilation preserve the vertical family |
| Wire session, terminal and checkpoint vocabulary | `architecture/wire/types.conduit` | generated at build time | yes | yes | exact native round trips plus wire, no-std and standalone firmware proof |
| Image observation reference | `semantics/human/types.conduit` | generated at build time | yes | yes | exact resource/dimension bounds and round trips plus catalog, hosted vision and image-text consumers |
| Four-slot tabular schema, row, result and query-outcome family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact fixed-collection and payload round trips plus deterministic provider, filter and materialized-result suites |
| Measurement plot profile and bounded 32-point series | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native bounds and round trips plus retained accounting validation, wire compatibility and browser consumer proof |
| Human interaction refusal, availability, outcome, range and quantization vocabulary | `semantics/human/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus interaction behavior suites |
| Speech-recognition attempt and result family | `semantics/tongues/types.conduit` | generated at build time | yes | yes | exact bounded identity/text/digest and payload round trips plus recognition and JSON-adapter suites |
| House generation request | `semantics/tongues/types.conduit` | generated at build time | yes | yes | exact identity/prompt/output bounds plus House, Tongues and std Host consumer suites |
| Chat conversation-request evidence | `semantics/chat/types.conduit` | generated at build time | yes | yes | exact request-identity bound, native and JSON round trips plus Chat and Patchbay consumer suites |
| Presentation place/aspect/depth, navigation, wardrobe, Face-role and narrator vocabulary | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native/code round trips plus navigation, mask, Face and generative-presentation suites |
| Presentation application action | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact nonempty 48-byte identity bound and event-kind composition plus unchanged application-view, Patchbay, Tour and ConduitOS consumers |
| Presentation fixed layout rectangle, eight-child frame and refusal family | `semantics/presentation/types.conduit` | generated at build time | yes | yes | exact native round trips plus preserved layout operations, fixed codec, Patchbay and ConduitOS consumers |
| Presentation static geometry family | `semantics/presentation/types.conduit` | generated at build time over the canonical `Quantity` leaf | yes | yes | native round trips for points, vectors, extents, rectangles, translations, poses, image regions and the product four-point path plus Presentation and std geometry suites; the runtime-count path constructor remains an explicit checked schema adapter |
| Portable purpose-state and fulfillment-readiness projection extracted from the catalog | `semantics/purpose/types.conduit` | generated at build time | yes | yes | exact 32-obligation, eight-evidence and 32-reason fixed bounds plus catalog/Form proof; Body lifecycle and fulfillment objects remain architectural runtime truth |
| Presentation rhetorical composition kind and bounded relation record | `semantics/presentation/types.conduit` | generated at build time with legacy Serde adapters | yes | yes | exact native payload/record round trips and 256-byte identity bounds plus preserved JSON/postcard shape, Presentation digest, Face, aural and browser consumers; subject membership, distinct endpoints and relation uniqueness remain contextual graph validation |
| AI similarity-score family | `semantics/ai/types.conduit` | generated at build time with exact finite `F32` | yes | yes | bit-exact native and JSON round trips plus vector retrieval/search suites and non-finite refusal proof |
| Alife field identities, reaction-diffusion cell and evolve-request family | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native/refinement round trips plus retained wire codecs and full Alife behavior suites |
| Lenia and reaction-diffusion portable value refusals | `semantics/alife/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus explicit separation from codec, worker, partition and assembler failures |
| Artificial-life family terminal ownership review | `semantics/alife/types.conduit` | generated at build time | yes | yes | all portable Lenia and reaction-diffusion parameters, identities, cells, requests, regions, partitions and value refusals are native; surviving engines, field generations, region work/results and boundary exchange state are M, while borrowed frames, chunk assembly and codec refusals are W |
| Audio family terminal ownership review | `semantics/audio/types.conduit` | generated at build time | yes | yes | every portable pitch, musical event, instrument control/mapping, timing, PCM profile/header, demand, compatibility and terminal value is native; surviving clip/frame/error declarations are exact codec views and conversion failures classified W |
| Finance family terminal ownership review | `semantics/finance/types.conduit` | generated at build time | yes | yes | every portable currency, decimal, money, rate, quote and transaction value is native; surviving fixture and mixed operation/adapter refusal are classified C/W |
| Robotics family terminal ownership review | `semantics/robotics/types.conduit` | generated at build time | yes | yes | every portable observation and navigation value is native; surviving structured fixture, refusal and catalog contract declarations are classified C |
| Signal family terminal ownership review | `semantics/signal/types.conduit` | generated at build time | yes | yes | every portable signal, trigger and finite configuration value is native and compiles through fixed no-std carriers; surviving encoders are M/W |
| Time family terminal ownership review | `semantics/time/types.conduit` | generated at build time | yes | yes | every portable instant, interval, pulse, replay, timeline, calendar, recurrence, schedule and pattern value is native; surviving bounded controllers/stores are M, borrowed request views and fixtures are C, and explicit codecs/refusals are W |
| Chat family terminal ownership review | `semantics/chat/types.conduit` | generated at build time | yes | yes | every portable message, history, summary, request-evidence, presentation-state, speech-evidence and conversation-stage value is native; surviving prompt/state orchestration is M, structured-value projections are C and specimen fixtures remain C |
| Text family terminal ownership review | `semantics/text/types.conduit` | generated at build time | yes | yes | every portable Morse, address and bounded pattern/result value is native; surviving interpreters are M, catalog/sample construction is C and byte codecs are W |
| Network family terminal ownership review | `semantics/net/types.conduit` | generated at build time | yes | yes | every portable transport, DNS, endpoint, attachment, delivery and transcript value is native; surviving bounded queues/trackers are M, borrowed codec views are C and protocol codecs/refusals are W |
| Tongues family terminal ownership review | `semantics/tongues/types.conduit` | generated at build time | yes | yes | every portable speech request, segment, recognition, commit, output-condition and conversation-boundary value is native; surviving recognizers, acoustic windows and committers are M, planning/specimen receipts are C and research reports/codecs are C/W |
| Web family terminal ownership review | `semantics/web/types.conduit` | generated at build time | yes | yes | every portable HTTP request/response, target, header, body and typed refusal is native; canonical bounded JSON travels as `value/json@1`, while its checked recursive parser tree and Kind builder are C, server transaction correlation is M and whole-document/HTTP encoders are W |
| Human generalized-input family | `semantics/human/types.conduit` | generated at build time | yes | yes | button transitions/state, axes, fixed slots, pointer position/delta, touch contacts, rotary steps, gamepad state and pressure evidence are native; catalog fixtures retain only range validation and deterministic sample construction |
| Tongues speech-message boundaries | `semantics/tongues/types.conduit` | generated at build time with the established JSON adapters | yes | yes | native source owns speakable segments and committed user messages with bounded identities/text and exact commit reason; streaming segmentation and commit evidence remain realization truth |
| Presentation vision family | `semantics/presentation/types.conduit` | generated at build time | yes | yes | image resources, pixel extent and format, color profiles, evidence and provenance, keypoints, landmarks, color samples, detections, motion, objects, visible text, tracks, impressions, observations, relations and complete experiences are native; the family reuses Human image references and Time instants, while catalog Rust retains only contextual validation and deterministic fixture construction |
| Process bounded-Job family | `semantics/process/types.conduit` | generated at build time | yes | yes | exact request, argument/environment, output, pressure, usage, lifecycle, refusal, exit and terminal-outcome round trips; exact eight-slot and 65,536-byte bounds; planned host/boot/generation/resource/authority enforcement and adversarial std effect-boundary proof |
| Dataset descriptor, example identity/page and split-membership family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native round trips, preserved flat semantic digest and full 4,096-member proof through compact bounded pages without exceeding the structured-node ceiling |
| Scientific observation identity and measured/derived provenance family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native payload round trips plus retained corpus validation, alignment derivation and historical semantic digest |
| Tensor axis, backing, value, summary and resource-identity family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native shape, axis, unit and payload bounds; payload-rich inline/resource backing; native structured round trips; preserved canonical tensor codec, sampled-signal, scientific-corpus, AI, audio and hosted-model behavior |
| Sampled-signal cadence, signal and concatenation family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native clock, positive-count, cadence, tensor, bounded shape/axis and 64-part meaning; native structured round trips; preserved semantic digest, window, concatenation, scientific, AI and audio behavior |
| Scientific observation, set, missingness, coordinate, calibration and aligned-view family | `semantics/data/types.conduit` | generated at build time | yes | yes | exact native identity/text/collection bounds, payload-rich tensor/signal values and nested round trips; preserved observation, corpus, alignment and semantic-digest behavior; every native length unit is admitted for coordinate frames |
| Immutable data-generation refusal family | `semantics/data/types.conduit` | generated at build time | yes | yes | exhaustive native terminal and nested reference-refusal round trips plus immutable generation, prepared-storage, text-front and std Host behavior |
| Data family terminal ownership review | `semantics/data/types.conduit` | generated at build time | yes | yes | every portable measurement, tensor, sampled-signal, tabular, dataset, scientific-observation and immutable-generation value is native; surviving stores, bounded state machines and collectors are M, borrowed rows/derivations are C, and explicit reference/measurement codecs and their refusals are W |
| Human family terminal ownership review | `semantics/human/types.conduit` and `semantics/presentation/types.conduit` | generated at build time; Catalog performs the exact Human construction-view to native Presentation vision conversion | yes | yes | every portable keyboard, generalized-input, image, image-text, experience Fore input/projection, refusal, interaction family/value/proposal/outcome and media-kind value is native; visual authoring records and source adapters are C, current-experience stores, interaction flows and keymap state are M, media realization/resource/authority lifecycle is R/M, and image-text encoding is W |
| AI wired House context request family | `semantics/ai/types.conduit` | generated at build time with the established JSON boundary | yes | yes | native source owns nonempty bounded item/value/source identities, 16,384-byte values, sixteen-item requests, 4,096-byte utterances and positive bounded output; Rust retains duplicate and aggregate-byte validation plus the established SHA-256 request identity |
| AI LLM work bounds | `semantics/ai/types.conduit` | generated at build time with retained Copy/Serde behavior | yes | yes | native construction owns exact input, context, output, work and history maxima; semantic contracts, provider discovery, planners, Patchbay, browser review and std Host consumers use checked construction/accessors |
| AI source-extraction finite limits | `semantics/ai/types.conduit` | generated at build time | yes | yes | native construction owns six positive full-U32 source, item, chunk, output and work limits; the obsolete post-construction `ZeroLimit` refusal is gone while relational extraction, resource access and receipt evidence remain Rust-owned at their reviewed P/R boundary |
| AI stochastic provenance | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns exact 32-byte model/query identities, optional checkpoint identity, randomness profile and draw relationship; Rust retains nonzero-identity validation and the established byte-exact probabilistic digest |
| AI probability summary | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns the closed samples/weighted-samples/trajectory-alternatives claim vocabulary, result count, exact model/query identities, randomness and disposition; handwritten static-string profiles are gone |
| AI log-probability value | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns the exact-millionths score, score kind, fixed support identity, stochastic provenance and disposition; Rust retains the probability-mass sign law, nonzero identity validation and established semantic digest |
| AI integration resource envelope | `semantics/ai/types.conduit` | generated at build time with retained Copy behavior | yes | yes | native record laws own positive state, context, output, step, evaluation, work and memory bounds plus the exact 2–65,536 output-sample range; obsolete post-construction zero/resource validation is gone while realization accounting remains Rust-owned |
| AI shadow resource envelope | `semantics/ai/types.conduit` | generated at build time with retained Copy behavior | yes | yes | native record laws own positive run, input, output and work bounds; the learned-lifecycle validator now compares evidence to an intrinsically valid envelope instead of rechecking construction |
| AI training resource envelope | `semantics/ai/types.conduit` | generated at build time with retained Copy behavior | yes | yes | native record laws own positive model, memory, compute, batch, step, work and checkpoint bounds, the 4,096-item batch ceiling and exactly one in-flight step; mutation-based invalid-resource fixtures and the handwritten validator are gone |
| AI supported relation-query pattern | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns nonempty 32-member evidence/target identity sequences with nonempty 128-byte members, exact mode/result profile and positive work/output bounds; Rust retains duplicate, disjointness and signature-membership laws |
| AI relation candidate output | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns the bounded target variable, exact 32-byte value identity, probabilistic disposition and sample count; Rust retains requested-profile/sample-count coherence and nonzero digest validation |
| AI training objective | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns nonempty 128-byte role, configuration and output identities, fixed-point weight, participation, and the law that optimized objectives have positive weight; Rust retains session-wide uniqueness and at-least-one-optimization laws |
| AI training metric | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns the bounded objective-output identity and exact signed-millionths value; Rust retains receipt-level uniqueness and objective-membership laws |
| AI retrieval chunk identity | `semantics/ai/types.conduit` | generated at build time with retained Copy, order and hash behavior | yes | yes | native source owns the exact 32-byte digest shape; established lineage hashing, source-extraction codec bytes, retrieval ordering and grounding consumers retain their behavior |
| AI rerank observation | `semantics/ai/types.conduit` | generated at build time with retained Copy behavior | yes | yes | native source owns the exact chunk identity, signed scorer-local ordering value and positive work bounded at 1,048,576; the obsolete zero-work runtime refusal is gone while candidate membership, uniqueness and aggregate-work laws remain Rust-owned |
| AI selected-context cost and omission | `semantics/ai/types.conduit` | generated at build time with retained Copy behavior | yes | yes | native source owns exact byte/token/work accounting and chunk-specific omission reason records; selection policy, accumulation and budget comparisons remain Rust-owned |
| AI context-selection disposition | `semantics/ai/types.conduit` | generated at build time | yes | yes | native source owns complete selection versus an explicit nonempty bounded set of omitted candidates; Rust retains the selection algorithm and budget decisions |
| Remaining P families in `semantics/**` | family-owned `.conduit` source required | binding machinery available | in progress | in progress | exact std/browser/ConduitOS/embedded applicability per family |

The completed foundations remove any general “language support” excuse for a
handwritten P declaration. Generated generic bindings, payload-rich variants,
record refinements and full-size bounded byte carriers are available. Checked
`Bytes <= 65536B` values now retain that exact bound without stack-sized native
frames, and the canonical structured envelope has finite room for the payload
and its framing. Remaining blockers must name the exact payload shape, target
constraint or unresolved Fore ownership question. As migrations land, add
reviewed family receipts and keep the classification history rather than
erasing it.

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
