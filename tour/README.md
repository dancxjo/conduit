# Tour of Conduit content

Live site: <https://dancxjo.github.io/conduit/>

The default route opens a reader-facing book cover and cumulative project table
of contents. `tour/book/current.json` is the one current pre-release reader
manifest (`schema_version: 0`): it owns named projects, chapters, sections and
ordered narrative blocks. Labs reference the separate machine-facing lesson
manifest by exact lesson id. Reference panels and Cookbook recipes have their
own searchable directories and do not participate in Previous/Next book
navigation.

Each book section reads in the order prose, action, compact real lab, result and
explanation. Expanding a lab reuses the same source, Patchbay session and result
instead of opening a teaching-only graph. Availability, contracts, exact plan,
accounting and evidence begin collapsed and retain addressable drawer ids.
Reading position and project checkpoints use reader-specific local storage;
learner source drafts keep their existing lesson-specific keys, and neither is
presented as a surviving live run.

Lesson source is real `.panel` input. This directory deliberately contains no
tutorial-only parser, graph, port model, or runtime. A browser delivery layer
must invoke the production parser/lowering/resolver/runtime artifacts and may
only persist learner drafts separately from authored source and exact plans.

The static page verifies its generated JS/WASM artifact plan, consumes the
browser host observation/resolution contract, and runs production Conduit WASM
inside a bounded dedicated-worker placement. Stop terminates that exact worker
and the page exposes bounded lifecycle evidence. Main-thread parsing and
Patchbay presentation remain separate from execution placement.

Patchbay opens a finite-history Rust workspace and renders only the versioned
`PatchbayViewModel` returned by WASM. The model carries parser/resolver topology,
explicit composite exports, exact bindings when available, provider
availability, presentation identity, and observed run/evidence/high-water
facts. ReactFlow owns layout and interaction only: it does not parse `.panel`,
invent ports or activity, or rewrite source.

Source replacement, configuration, connection, disconnection, and movement
use typed candidate transactions with expected source and presentation
revisions. Rust validates candidate source through the parser and resolver and
returns bounded diagnostics, a compatibility proof, a candidate exact-plan
identity when one can be resolved, and a committed/rejected disposition.
Presentation failure does not remove the editor, checker, or worker controls.

Every lesson and reference panel declares `runnable`, `contract-only`, or
`illustrative/unavailable`. Only `runnable` sources enable Run. Reference
panels fetch their canonical checked-in `examples/*.panel` source; unavailable
providers produce the declared structured resolver diagnostic. Pedagogical
check completion is recorded separately and is never presented as executor
evidence.

Library lessons add a checked descriptor with standalone, composition, failure,
and cancellation scenarios. Their execution-story controls only reveal,
select, and explain the ordered evidence returned by the exact Rust run. The
Patchbay projection and timeline therefore share one plan/run/evidence
authority. Play, pause, step, reset, replay, and scrub are presentation state;
the complete ordered table remains available without motion, color, or audio.

Platform lessons reuse that surface for checked plan profiles. Admission
outcomes come from repository conformance fixtures, while the editable
representative panel still runs through the exact browser worker. Envelope,
clock-conversion, and feedback facts remain exact-plan projections rather than
teaching-only runtime events.

`tour/build-site.sh` assembles the checked static Tour, lessons, exact
browser-host adapter, linked examples, and the formatter's linked
specification/cookbook/conformance fixture. The Pages workflow publishes that
artifact only after the corresponding `main` CI run succeeds.
