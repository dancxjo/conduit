# Tour of Conduit content

Live site: <https://dancxjo.github.io/conduit/>

The default route opens a reader-facing book cover and cumulative project table
of contents. `tour/book/current.json` is the one current pre-release reader
manifest (`schema_version: 0`): it owns named projects, chapters, sections and
ordered narrative blocks. Labs reference the separate machine-facing lesson
manifest by exact lesson id. Reference panels and Cookbook recipes have their
own searchable directories and do not participate in Previous/Next book
navigation.

The prologue leads into three cumulative builds: a browser-local living
instrument, a bounded service, and a no-actuation robot rehearsal. The checked
`tour/book/migration.json` ledger gives every machine lesson exactly one Book,
Interlude, Cookbook, Reference, or Retire/Replace disposition and maps old
lesson routes to their one current destination. `fresh-reader-study.json`
defines the independent-reader protocol and stays `protocol-ready` until real
participant observations exist.

Each book section reads in the order prose, action, compact real lab, result and
explanation. Expanding a lab reuses the same source, Patchbay session and result
instead of opening a teaching-only graph. Availability, contracts, exact plan,
accounting and evidence begin collapsed and retain addressable drawer ids.
Reading position and project checkpoints use reader-specific local storage;
learner source drafts keep their existing lesson-specific keys, and neither is
presented as a surviving live run.

Project revision state is also reader state, not executor evidence. Reset saves
one explicit recovery snapshot containing the project revision, checkpoints,
and owned lesson drafts; Recover restores that snapshot. The living
instrument's beat/light figure is driven only by the production executor's
exact public Watch value, has an adjacent ordered-text equivalent, and does not
run a decorative timer or activate audio.

The project explicitly contrasts a standing patch with an imperative loop:
the graph remains present and live, pulses advance state, cords carry typed
values and events, explicit delay or memory gives feedback temporal meaning,
and lifecycle control starts and stops one immutable plan epoch.

The standing-network project likewise begins with two isolated application
endpoints and adds link/address readiness, DHCP, a local name, a route, a
listener, bounded frame/packet/datagram/stream exchanges, observation, failure,
and recovery in explicit stages. Patchbay duplicates every network family with
text and jack/cord shape, not color alone. Its full assembled topology is a
checked design; the smaller runnable stages keep each exact run inside the
finite scheduler-evidence window.

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

`tour/browser-plan-contract.json` is the tracked semantic contract and finite
browser bound. Build-specific wasm-bindgen outputs and `browser-plan.json` are
not source files: `bash tour/build-artifact.sh` creates them under
`target/tour-runtime`, assembles the complete site under `target/tour-site`,
and emits a deterministic release archive plus checksum under
`target/tour-dist`.

The three Playwright engine jobs download one commit-named CI artifact and
serve its assembled site as an overlay on the source-owned test harness. Each
engine keeps one worker, has no retries or per-test timeout overrides, stops at
its first failure, and must finish inside the workflow's suite and job bounds.
For the same path locally, run `npm ci` followed by
`npm run test:browser:local`.

The Pages workflow downloads the successful `main` CI artifact instead of
checking out or rebuilding it. A `v*` tag publishes the same tested archive
and checksum as an attested GitHub release artifact.
