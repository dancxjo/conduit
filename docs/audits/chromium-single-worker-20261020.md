# Single-worker Chromium repair (#6172)

The accepted repair #6173 restores one Chromium batch by default; the failed
concurrent experiment stays opt-in. The pipeline and guide on dev `6a8e127b4`
are byte-identical to repair head `9ef231be7`. No reviewed spec, product binding,
worker limit, retry policy or evidence validator is weakened.

[Candidate run 39510280635](https://github.com/dancxjo/conduit/actions/runs/39510280635)
completed the browser job with **263 actual passing tests across all 31 reviewed
specs**, one batch, one worker and zero retries. The complete retained job log
states `263 passed (21.4m)`. This is actual Chromium execution, not an inference
from a green aggregate badge. The checkout recorded by the product receipt is
synthetic merge `e3fc8593ce0d42aa7f65cb60fa9b5c644c73522b`, with parents
`c93a4cca2ac48405415ce813e43e973d93005bad` and repair head
`9ef231be7b4bd2227b86dd0d0fd9b6c323ba1782`. It is not relabeled as current dev.

The official `product-browser` artifact 12307079878 ZIP has SHA-256
`1c9802615db939d1668a3e55090382e5998dee80b3b659e467882b1f74ef87d8`.
Every one of its **1,095 manifest-listed files** was independently checked for
exact size and SHA-256. Its retained scope agrees on the inventory and execution
settings. The complete archive remains locally retained and available from the
run; the smaller [source, scope, manifest and full log records](evidence/chromium-single-worker-20261020/manifest.json)
are committed. An empty `gh run view --log` response was not treated as evidence;
the complete job log was obtained from the job-log API instead.

All **18 current focused runtime/network tests** pass on `6a8e127b4` using
`node --test proof/ci/pipeline-browser-runtime.spec.mjs proof/ci/pipeline-browser-network.spec.mjs`.
Their scope is distinct from the 263 browser tests. The original failed
Integration 39500212975 and diagnostic artifact 12304194618 remain failed and
preserved in #6172; this repair does not reinterpret them or assert a proven
contention cause.

The user explicitly directed bypassing standard proof/CI conventions and direct
dev delivery. #6172 closes the merged repair and complete Candidate execution;
the additional exact-head combined Integration gate is bypassed under that
instruction. Combined Integration and stable publication are not asserted.
