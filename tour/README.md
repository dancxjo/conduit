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

`tour/build-site.sh` assembles only the checked static Tour, lessons, exact
browser-host adapter, and license. The Pages workflow publishes that artifact
only after the corresponding `main` CI run succeeds.
