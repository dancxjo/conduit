# Tour of Conduit content

Lesson source is real `.panel` input. This directory deliberately contains no
tutorial-only parser, graph, port model, or runtime. A browser delivery layer
must invoke the production parser/lowering/resolver/runtime artifacts and may
only persist learner drafts separately from authored source and exact plans.

The current static WASM page is a bounded local execution proof, not a complete
browser-host implementation. It does not claim worker, service-worker,
AudioWorklet, WebGPU, permission, fresh-host-report, distributed-cord, or
cross-browser conformance behavior; those remain the reopened #86 boundary.
