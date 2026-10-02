# Project status and roadmap

Conduit is experimental. The development tree includes checked Conduitese,
immutable planning, one bounded kernel, body lifecycle, hosted/browser/ConduitOS
execution, and environment-specific evidence. Those are different proof claims,
not a promise of identical support on every host.

## Find the current answer

| Question | Source |
|---|---|
| What is implemented, and with what limits? | [STATUS](https://github.com/dancxjo/conduit/blob/dev/STATUS.md) |
| What is unfinished or paused? | [Roadmap](https://github.com/dancxjo/conduit/blob/dev/docs/roadmap.md) and [open issues](https://github.com/dancxjo/conduit/issues?q=is%3Aissue+is%3Aopen) |
| What code does the published product contain? | [Current product truth](https://dancxjo.github.io/conduit/current-product.html) |
| What syntax can I write? | [[Current language surface|Current-language-surface]] |
| What does a test or screenshot establish? | [[Evidence and proof|Evidence-and-proof]] |
| Where is an old acceptance record? | [History](https://github.com/dancxjo/conduit/blob/dev/docs/history/README.md) |

Development implementation, accepted release, and published product can differ.
An open issue is unfinished or planned work; it does not establish that someone
is actively implementing it. Follow an issue for its latest scope and state.

## Language boundaries worth knowing

- Executable `plot` and portable encoding `code` are current spelling; the `plot`/`plot` pair in [#4513](https://github.com/dancxjo/conduit/issues/4513) is proposed
- [#4378](https://github.com/dancxjo/conduit/issues/4378) completed bounded each/select/fold/scan and collection, with scoped std/browser runtime proof and an explicit embedded applicability limit
- Record `where` laws are enforced at generated construction boundaries; [#4639](https://github.com/dancxjo/conduit/issues/4639) still owns general propagation into consuming-plot arithmetic proofs
- [#4375](https://github.com/dancxjo/conduit/issues/4375) tracks the remaining native semantic ownership migration

Use the reference for exact examples and evidence. This page routes to current
owners rather than keeping a competing release diary.
