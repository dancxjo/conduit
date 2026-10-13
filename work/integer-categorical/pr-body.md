This adds reusable integer categorical scoring over an exact immutable model resource, with an ordinary bounded Kernel Step Back/factory and hosted adapter. Dimensions, model identity, resource adoption, frame limits and acknowledgement-controlled progress remain explicit. It contains no Language policy, trained weights, speech implementation or public Session.

Stacked on `codex/portable-foundations`. The implementation and tests preserve the original source identities; the final commit adds byte-exact validation logs and hashes only.

Validation: 10 runtime tests pass, covering score/resource parity, scheduler reuse after provider drop, pressure, cancellation, malformed input, storage failure and Value/Flow completion. Strict scoped AI Clippy and hosted-library Clippy pass, as do formatting and diff checks. Tested source: `fb2ffdb591be15fceeb47ff8f0a713de4ea6c7f8`.

Preparation-wide allocation accounting remains an inherited limitation. Allocation-free runtime checks do not establish a complete preparation bound or linguistic acceptance.
