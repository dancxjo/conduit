# Tour of Conduit content

Live site: <https://dancxjo.github.io/conduit/>

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

`tour/build-site.sh` assembles only the checked static Tour, lessons, exact
browser-host adapter, and license. The Pages workflow publishes that artifact
only after the corresponding `main` CI run succeeds.
