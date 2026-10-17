# Two-Ollama-Host acceptance (#5552)

The supported `cargo xtask prove local-model-pool` entrance retains deterministic
Plan/Play and provider-loss evidence separately from an optional imported live
receipt. Fixture execution does not establish a real two-machine session.

## Current live prerequisites

A read-only preflight on 16 October 2026 found Ollama at the local loopback
endpoint and `forebrain.local:11434`. Both inventories reported `gemma3:latest`
with digest `a2af6cc3eb7fa8be8504abaf9b04e88f17a119ec3f04a3addf55f92841195f5a`
and 3,338,801,804 model bytes. The services reported versions 0.23.2 and 0.34.1.
Only GET `/api/version` and GET `/api/tags` were used; no inference or download
was performed. This is provider metadata, not eligible Host advertisements.

The existing batch SSH credentials could not launch a second executing Conduit
Host: `forebrain.local` refused authentication with
`Permission denied (publickey,password)`. A reachable provider API does not
replace an executing Host with an exact Boot, admitted resources and planned
session lines. The native Ollama adapter intentionally accepts an exact
loopback origin; pointing a local Host at the remote HTTP service would not
prove two Hosts and is not a substitute for the missing execution prerequisite.

Installed model selection also initially offered presentation only. The installed Host
`--model-generation` option adds explicit bounded generation offers and retains
the complete initialized profile set across fresh Boots. That offer alone does
not admit a pool or a request.

## Evidence still required for live timing

No live two-Host receipt or measured short/long request comparison has been
produced. A real run must retain checked Source, expanded Plot, exact immutable
Plan and Play, distinct executing Host/Boot identities, selected model artifact,
current finite resource offers, admitted session bindings, request prediction
and completion measurements, and provider refusal/disappearance Signs.
Adapter-call duration must remain separate from queue, warmup and transfer
measurements unless those phases were independently observed.

Deterministic demand-sensitive forecasts, capacity-reference comparisons and
provider-loss cases remain named fixtures. They cannot establish measured
Ollama throughput, timing accuracy or a two-machine speedup. Parent #5228 and
the live workload capstone #5575 retain their original acceptance obligations.

## Retained evidence and acceptance status

The [manifest](evidence/local-model-pool-5552/manifest.json) pins the supported
proof run to its exact source commit and the digest of its
[deterministic receipt](evidence/local-model-pool-5552/receipt.json).
Its Plan/Play identities, provider-loss and no-replay assertions are fixture
results. The [service preflight](evidence/local-model-pool-5552/service-preflight.json)
is a separate observation and is not an imported live execution receipt.

For #5552, the unavailable remote execution prerequisite is documented above;
the attempted SSH route did not establish a second worker. This does not rule
out other authorized execution routes. A live short/long comparison remains
unmeasured, so there is no measured improvement over utilization-only selection.
Provider disappearance/refusal and retained Plan identity have deterministic
coverage only. Keep these limits attached to any issue acceptance decision;
this document does not establish parent #5228 or capstone #5575 completion.
