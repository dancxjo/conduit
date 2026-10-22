# Retained Graph reply and projection staging

[#6213](https://github.com/dancxjo/conduit/issues/6213) and [PR #6214](https://github.com/dancxjo/conduit/pull/6214) separate the protected Application's bulk Graph reply and projection preparation into ordered entries under the existing quantum and storage grants. Exact clean source **`e589550cac0d43cafe575fc792c6729ac80ee802`** passed the full supported graphical-profile proof locally. This audit is a documentation successor, not the executed source.

## Final behavior

`Graph` Commit retains the selected query. Separate requests serialize a borrowed graph, validate immutable framing, and pack the reply in turns consuming at most 4 KiB of decoded input. The selected graph is not cloned. Pending state refuses interleaving, skipped/duplicate phases, premature reads, wrong packing offsets and unread-output replacement. Root decodes the existing typed Graph reply and checks equality.

`PrepareGraphProjection` now uses the existing immutable upload Preflight before Commit. Decompression and framing validation run in one entry; typed graph preparation consumes those frozen validated bytes in the next. Writes after Preflight refuse. The existing Finish still validates the exact graph custody digest before publishing a contribution. No new queue, authority, fallback, retry, timeout, quantum or memory grant is introduced.

The narrow Plot-selection collector reports complete typed terminal kernel refusals before accepting a revision, retaining the packet and actual target Plot. Partial tails and recoverable action refusals remain distinct.

## Earlier proof and failures remain separate

[Candidate 39684672200](https://github.com/dancxjo/conduit/actions/runs/39684672200/job/119516116334), source `7a6294a06bc81b14673e8aedb7721b0b48a794b8`, and [Integration 39686810134](https://github.com/dancxjo/conduit/actions/runs/39686810134/job/119523587773), source `3672758cf3f8bbaec01dab14476882433d31621f`, exhausted monolithic Graph Commit at sequence 88. Their [original serial/QMP/build packets](evidence/staged-graph-reply-20261021/negative/) are retained.

The [earlier local source `9cfc7ff59`](evidence/staged-graph-reply-20261021/journey-proof.json) completed its 24-step journey and three reply phase sequences; its [36 core/five collector tests and original captures](evidence/staged-graph-reply-20261021/) remain bound to that source. That local success did not establish hosted acceptance. [Candidate 39694679470](https://github.com/dancxjo/conduit/actions/runs/39694679470/job/119549657220), tested merge `1ae7f28daa4fb57d853eaee7c7736287fd5dfd65`, passed two reply phase sequences but exhausted the later `PrepareGraphProjection` Commit at sequence 207, ending with `front-door-face-refused`. The corrected evidence retains this original failure too.

## Corrected exact-source result

The [permanent corrected evidence](https://github.com/dancxjo/conduit/blob/8254d4afec527dd358861bc34f747e8bd048f201/docs/audits/evidence/staged-graph-reply-20261021/corrected-projection-e589/README.md) preserves source `e589550ca`, the original receipt, 24 PNGs, raw serial/QMP, test logs and hashes. All **39 Application-core tests** passed. The production allocator regression retained peak **2,040,832 bytes** within the unchanged **2,072,576-byte** grant, leaving **31,744 bytes** of rounded live headroom. Full workspace formatting passed. An initial `77ba1ca79` test-fixture type error occurred before execution and is preserved separately from the corrected success.

`cargo +stable xtask make conduitos graphical-profile-proof --locked` exited **0**: the normal x86_64 product completed all 24 steps with four complete projection Preflight/preparation pairs, three complete reply sequences and no terminal kernel refusal. All four other images passed graphical-asset absence checks. The final receipt matches the clean tested source and original normal ISO hash.

## Proof boundary

These are local CPU and freestanding x86_64 emulator results. The four headless images were checked for asset closure, not booted as runtime proofs. Full serialization, validation, decompression and typed decode passes still require their existing finite admission; the recorded workload demonstrates fit, not all admitted graphs. Compression retains internal buffered work. No current-head Candidate, exact-head Integration, reviewed merge, stable publication, physical execution, human enactment or issue closure is claimed. Earlier negative evidence is not superseded into a pass.
