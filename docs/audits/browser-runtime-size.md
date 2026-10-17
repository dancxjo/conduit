# Browser runtime package-size repair

Two consecutive Integration browser jobs stopped before Chromium because the
linked runtime exceeded the Workspace application's existing 20 MiB resource
limit: 21,451,720 bytes against 20,971,520. Other selected unit lanes passed.
The source tested here is `42a571fd02308b35528879fe684e402e280eeb73`.

## Change

Keep the existing whole-graph Rust release settings (`opt-level=z`, fat LTO,
one codegen unit). The browser release builder now runs Binaryen `wasm-opt -Oz
--enable-bulk-memory --enable-sign-ext` on the linked module before sealing release file digests.
It preserves the Cargo output for diagnosis and fails if optimization fails;
there is no unoptimized fallback. Browser CI acquires Binaryen, and the browser
doctor reports its availability. Application and release bounds are unchanged.

## Controlled artifact measurement

The exact retained local input is 21,454,248 bytes, SHA-256
`9973540aca9a4ebce43328270cd3f7c539e2449e84f055754de155ce45de54ed`. The local artifact is not byte-identical to the
failed CI artifact; all comparisons below use this one retained local input.
Binaryen 120 (Ubuntu package 120-4build1), with two optimizer workers, produced
19,175,029 bytes, SHA-256 `5368fdcb59965856967d3d3f2eac837ffa5ad9c135b02900a0b24db308d80bdf`.
That saves 2,279,219 bytes (10.62%) and leaves
1,796,491 bytes under the unchanged application limit.
The optimizer used 70.85 user CPU seconds, 36.60 wall seconds and 645,036 KiB
maximum RSS. This is an artifact-size result, not a measured whole-CI speedup
or runtime throughput improvement.

## Execution evidence

Node WebAssembly execution compared complete effects and terminal receipts for
60 cases before and after optimization. All imports and exports also matched.
The cases include the existing exact quantity corpus, affine conversions,
retained incompatibility/overflow refusals, authored Unit snapshots, receipt
comparators, computed and equivalent native value arguments, malformed history,
argument overflow, the actual packaged pitch Source and quoted-Quantity refusal.

The unchanged numerical/value browser specifications also passed in pinned
Playwright 1.62.0 Chromium: eight tests, one worker, zero retries. Their copied
test and fixture bytes were checked against current repository sources. This
proves optimized WASM execution in Chromium, not full packaged Workspace or
Handbook lifecycle acceptance. Integration still owns those gates.

Canonical release packaging with Binaryen 120 and strict affected-tool lint passed.
No accepted release is claimed by these development measurements.

## Ubuntu 24.04 compatibility

The official Noble package index supplies Binaryen 108 (`108-1`), not the
local version 120. Its package SHA-256 was checked against the official index
before extraction. It requires explicit sign-extension support to validate
operations already emitted by Rust, so the final builder declares both bulk
memory and sign extension; it does not enable fast math.

The same retained input produced 19,219,021 bytes with version 108,
SHA-256 `527b117cc5e607159737c3ca714226c929b4d88a13579c5d8cffa79bdd253d2a`. All 60 complete execution results and terminal receipts
also matched this output. The optimization used 81.59 user CPU
seconds, 0:42.57 wall time and 610912 KiB maximum RSS.
The package is indexed at [Ubuntu's Noble archive](https://archive.ubuntu.com/ubuntu/dists/noble/universe/binary-amd64/Packages.xz);
its exact filename and digest are retained in the accompanying evidence.

The version-108 output also passed the same eight pinned Chromium tests (one
worker, zero retries). An isolated call to the actual application package
builder retained the unchanged production runtime resource entry: the baseline
was refused and the optimized bytes were accepted with their exact digest.
This isolates the reported byte-bound failure; full staged-application lifecycle
proof remains the Integration lane's responsibility.
