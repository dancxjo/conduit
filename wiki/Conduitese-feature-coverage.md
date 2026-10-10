# Conduitese feature example index

Use this index to find an example of each reviewed language feature. It covers
the current parser/checker surface and language proposals in
[dancxjo/conduit's open issues](https://github.com/dancxjo/conduit/issues?q=is%3Aissue+is%3Aopen),
reviewed on **9 October 2026**. The issue list is the upcoming-work source;
completed issues provide provenance, not future work. Individual catalog Kinds
are extensible domain libraries, rather than separate grammar features.

An example labeled **current** demonstrates development grammar or a named
implementation boundary. A **fragment** assumes declared ports and available
Kinds. A **proposal** demonstrates intended behavior and is not runnable syntax.
Use [[Current language surface|Current-language-surface]] for proof limits and
[current product truth](https://dancxjo.github.io/conduit/current-product.html)
for the published product's exact release.

## Meaning, composition and execution

| Current feature | Example in this handbook |
|---|---|
| Live Plot and source Gear | [[Clock|Conduitese-by-example#clock-a-standing-live-plot]] |
| Finite-on-drain Plot `}.` | [[Hello|Conduitese-by-example#hello-a-finite-pipeline]] |
| Cord `>>`, anonymous Gear and explicit port | [[Fore directions|Plots-and-flow#cords-and-fore-direction]] and [[Memory Lantern|Conduitese-by-example#memory-lantern-input-editing-and-current]] |
| Paired Fore `input: T >> output: U` | [[Desk Telegraph|Conduitese-by-example#desk-telegraph-reusable-plots-as-gears]] |
| Required/default startup parameters; named/positional calls | [[Parameterized pure Plot|Conduitese-by-example#a-pure-plot-with-a-default-parameter]] |
| Reusable Plot as Gear | [[Desk Telegraph|Conduitese-by-example#desk-telegraph-reusable-plots-as-gears]] |
| Named compile-time Type parameters and inference | [[Specialization|Conduitese-by-example#specialize-a-reusable-plot]] |
| Exact behavior parameters and `activate` | [[Bounded each|Conduitese-by-example#bounded-collection-behavior]] |
| `select`, `fold`, `scan`, bounded collection on close | [[Bounded activation|Current-language-surface#bounded-each-select-fold-and-scan]] |
| Private nested Plot | [[Nested helper|Conduitese-by-example#a-private-nested-plot]] |
| Shared finite pool and explicit consumer binding | [[Pool webchat|Conduitese-by-example#one-shared-pool-explicit-consumers]] |
| Glyph aliases and `sans glyphs` | [[Glyph reference|Current-language-surface#gear-glyphs]] |
| Explicit fan-out and pressure law | [[Light plus tone|Conduitese-by-example#explicit-enrichment]] |
| Merge, zip, race, combine-latest, Current sample | [[Temporal relationships|Terminals-and-concurrency#multi-input-temporal-relationships-must-be-explicit]] |
| Routing variant cases, record guards and otherwise | [[Routing|Plots-and-flow#exactly-one-graph-routing]] |
| Unary `when` filter | [[Filter|Plots-and-flow#unary-filter-sugar-when]] |
| Typed `project`, fixed `index`, `select` with drop/refuse | [[Structured selectors|Plots-and-flow#glyph-composition-and-exact-selectors]] |
| Normal close `|`, abnormal terminal `!`, quiescence `;` | [[Endpoint tracks|Terminals-and-concurrency#normal-close-and-abnormal-terminal-projections]] |
| Cancellation request `~` and terminal propagation | [[Cancellation|Terminals-and-concurrency#semantic-cancellation]] |
| Inferred effect eligibility and semantic realization | [[Effects|Effects-and-realization]] with [[Body Chat|Conduitese-by-example#body-chat-application-meaning-model-choice-still-realization]] |

## Values, Types and pure computation

| Current feature | Example in this handbook |
|---|---|
| Native unit literals and dimension-aware consumers | [[Pocket Theremin|Units-and-quantities#a-distance-becomes-a-musical-pitch]] |
| All 24 SI prefixes, case and approved aliases | [[SI table|Units-and-quantities#all-24-decimal-si-prefixes]] |
| Exact conversion and comparison | [[Conversions|Units-and-quantities#convert-explicitly-and-retain-the-result]] and [[comparison|Units-and-quantities#compare-compatible-dimensions-exactly]] |
| Absolute temperature versus temperature difference | [[Temperature changes|Units-and-quantities#a-temperature-point-differs-from-a-temperature-change]] |
| Explicit wider quantity profile, inexact/overflow/dimension refusals | [[Extreme scale|Units-and-quantities#extreme-scale-needs-an-explicit-representation]] and [[refusals|Units-and-quantities#refusal-is-an-inspectable-result]] |
| Integers, IEEE floats, Boolean, text and byte bounds | [[Scalar expressions|Plots-and-flow#systems-grade-scalar-expressions]] and [[refinements|Current-language-surface#checked-refinements-and-portable-patterns]] |
| Decimal/hex/binary/octal literals and separators | [[Scalar literals|Plots-and-flow#systems-grade-scalar-expressions]] |
| Text quoting, escapes, Unicode and source comments | [[Text and source spelling|Plots-and-flow#text-and-source-spelling]] |
| Unary, arithmetic, comparison, equality, bitwise, shift and Boolean expressions | [[Complete expression Plots|Plots-and-flow#complete-expression-plots]] |
| Pure expression Plot, input `.`, field and tuple projections | [[Expression Plot|Conduitese-by-example#a-pure-plot-with-a-default-parameter]] and [[structures|Types-and-state#anonymous-finite-structures]] |
| Ternary evaluates one pure branch | [[Choice|Conduitese-by-example#choose-a-payload-bearing-variant]] |
| Immutable local dependencies and record punning | [[Locals|Plots-and-flow#immutable-locals]] |
| Checked semantic calls and explicit integer widening | [[Widening|Conduitese-by-example#give-an-arithmetic-invariant-to-the-type]] and [[bounded indexing|Conduitese-by-example#check-an-index-against-the-actual-length]] |
| Named/anonymous/nested records and tuples | [[Nested record|Conduitese-by-example#construct-a-nested-record]] and [[structures|Types-and-state#anonymous-finite-structures]] |
| Payload-bearing/payloadless finite variant constructors | [[Choice|Conduitese-by-example#choose-a-payload-bearing-variant]] |
| Generic native Type families and checked identity | [[Generic Types|Current-language-surface#generic-native-types]] |
| Finite U16 value parameters, shape arithmetic and independent sequence capacities (#5326) | [[Value parameters|Current-language-surface#finite-native-type-value-parameters-5326]] |
| Native scalar declarations; resolved identity versus source aliases | [[Identity|Types-and-state#checked-type-identity-is-not-source-spelling]] |
| Exact collections, bounded sequences, empty sequence and actual length | [[Finite containers|Conduitese-by-example#finite-collections-and-variable-length-sequences]] |
| Byte bounds, membership, excluded membership, closed/open-ended ranges | [[Refinements|Current-language-surface#checked-refinements-and-portable-patterns]] |
| Portable positive/negative text patterns, flags, conjunction | [[Patterns|Current-language-surface#checked-refinements-and-portable-patterns]] |
| `finite` floating refinement and scalar `where` law | [[Scalar laws|Conduitese-by-example#give-an-arithmetic-invariant-to-the-type]] and [[refinements|Current-language-surface#checked-refinements-and-portable-patterns]] |
| Record `where` laws and bounded arithmetic proof propagation | [[Interval|Conduitese-by-example#a-record-owns-its-law]] |
| Separate semantic Type and compact `u8` Form | [[Compact Forms|Current-language-surface#compact-forms]] |
| Value, optional value, flow, closing flow, Current, optional Current | [[Temporal shapes|Types-and-state#temporal-value-modalities]] |
| Initialized/uninitialized/optional `keep`, all six durations/default | [[Retained state|Types-and-state#keep-explicit-retained-current-value]] |
| Exact immutable data generation reference `&T` | [[Data references|Types-and-state#data-t-save-load-and-disk]] |
| Admitted resource possession `resource T` | [[Resource ports|Current-language-surface#runtime-bound-resources]] |

## Construction and distribution

| Current feature | Example in this handbook |
|---|---|
| Imports, grouped imports, ordinary and glyph aliases | [[Imports|Current-language-surface#imports-and-packs]] |
| Relative module imports | [[Packs and imports|Packs-hosts-and-bodies#packs-imports-resolution-and-distribution]] |
| Pack version, `ship`, `need`, exact lock and no authority grant | [[Pack|Conduitese-by-example#a-pack]] |
| Host target/build/loader, resource pools, Base, Back, policy and bounds | [[Host source|Current-language-surface#host-source]] |
| Body hosts, parts and spores; construction versus current membership | [[Body construction example|Packs-hosts-and-bodies#a-body-construction-source]] |
| Body unordered `wear` and optional ordered `want` | [[Wardrobe|Conduitese-by-example#body-wardrobe]] |
| Mask as ordinary Plot; Face, interaction and Show | [[Native graphical Mask|Conduitese-by-example#a-mask-is-an-ordinary-plot]] |

## Proposed language features

The scoped glyph family below remains a proposal. The implemented value-parameter
surface under #5326 is documented above. Its issue tracks independent
stable-release acceptance for the published product.

### Scoped typed delimiter glyph families — #5317

[Issue #5317](https://github.com/dancxjo/conduit/issues/5317) proposes locally
imported typed literal families:

```conduit
# PROPOSAL ONLY: typed delimiter families are not current grammar/exports.
with speech/ipa/notation as ph
with pattern/portable/notation as r

phonetic = ph[foˈnetika]
phonemic = ph/fonz/
```

`ph[…]` denotes universal phonetic transcription. `ph/…/` requires a supplied
checked phonemic inventory/variety/basis; the snippet deliberately omits that
basis and therefore does not claim a valid phonemic value. Both remain
transcription Types even for one segment. Single phones and phonemes have
separate explicit constructors.

The non-Speech example is the proposed refinement
`Text <= 64B ~ r/[A-Z]+/i`, with the same checked pattern meaning as current
`Text <= 64B ~ /[A-Z]+/i`. The domain owns lexical policy, bounds and payload
escapes. Conflicts with indexing or division refuse; expected return Type
cannot choose a parser. Raw source and original spans survive lowering into
an ordinary qualified constructor. Imports grant no speech or parser authority.
Native `440Hz`/`250ms` quantity suffixes stay import-free.

## Planned domain experiences written with the language

These related open issues add domain contracts, composition or product proof
rather than general language grammar. Each has a concrete example of the
intended work below. These are acceptance scenarios, not installed Kind APIs.

| Open feature | Worked intended example |
|---|---|
| [Acoustic quantities #5216](https://github.com/dancxjo/conduit/issues/5216), [Speech binding #5262](https://github.com/dancxjo/conduit/issues/5262) | [[200-Hz intent at three sample rates|Units-and-quantities#acoustic-quantities-current-building-blocks-and-planned-integration]]; preserve exact duration, referenced dB, overlap, uncertainty and linguistic scope |
| [Reusable DSP #5217](https://github.com/dancxjo/conduit/issues/5217), [phonological gestures #5218](https://github.com/dancxjo/conduit/issues/5218) | Excitation → resonator → filter → PCM; use an overlapping pitch/energy trajectory for one IPA utterance, then reuse the resonator in a musical patch |
| [Streaming #5222](https://github.com/dancxjo/conduit/issues/5222), [measurements #5223](https://github.com/dancxjo/conduit/issues/5223), [acoustic tutorials #5224](https://github.com/dancxjo/conduit/issues/5224), [EPIC #5215](https://github.com/dancxjo/conduit/issues/5215) | The same finite speech/synth patch renders successive blocks under pressure; inspect intended and measured pitch separately, then edit the authored patch in Patchbay |
| [Model Gear #5219](https://github.com/dancxjo/conduit/issues/5219), [ORT Back #5220](https://github.com/dancxjo/conduit/issues/5220), [Piper #5221](https://github.com/dancxjo/conduit/issues/5221), [voice journey #5225](https://github.com/dancxjo/conduit/issues/5225) | Checked utterance → verified model inference → bounded PCM → retained WAV; source keeps model meaning while the Host supplies ORT. See [[model contracts|Creating-models]] |
| [ConduitVoice #5206](https://github.com/dancxjo/conduit/issues/5206), [model tutorial #5207](https://github.com/dancxjo/conduit/issues/5207), [FARGAN #4898](https://github.com/dancxjo/conduit/issues/4898) | Train, checkpoint and resume a compact acoustic model, then realize its admitted output through a Plot-owned vocoder. [[Creating models|Creating-models]] separates existing recipe proof from this unfinished voice outcome |
| [Language revisions #4907](https://github.com/dancxjo/conduit/issues/4907), [common IR #5261](https://github.com/dancxjo/conduit/issues/5261), [IPA prerequisite #5212](https://github.com/dancxjo/conduit/issues/5212), [two realizations #5263](https://github.com/dancxjo/conduit/issues/5263) | Append text, replace a tentative span, commit the linguistic revision, then retain phone/syllable/source identities while selecting formant or neural realization; a committed occurrence cannot silently change identity |
| [Mask components #5365](https://github.com/dancxjo/conduit/issues/5365), [Face/component/Look separation #5364](https://github.com/dancxjo/conduit/issues/5364) | Reuse a checked prompt component in two Masks over the same Face action; substitute visual or spoken Look without changing the action identity. [[Native Mask|Conduitese-by-example#a-mask-is-an-ordinary-plot]] is the current composition foundation |
| [Commons Plot #5359](https://github.com/dancxjo/conduit/issues/5359), [cross-Mask invariants #5355](https://github.com/dancxjo/conduit/issues/5355) | Author a finite place with an inspectable door/action and encounter it through graphical and spoken Masks; both interactions must address the same verified Face input |

Scheduler optimization, USB implementations, confinement and presentation
polish are Host/runtime/product work. They can realize an authored Plot without
introducing a new language construct. Their own issue criteria still govern
completion; an example or a grammar check does not close those issues.
