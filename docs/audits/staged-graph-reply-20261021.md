# Retained Graph reply staging: local execution evidence

Issue [#6213](https://github.com/dancxjo/conduit/issues/6213) repairs a protected Application `Graph` query that exhausted its unchanged entry quantum during ordinary Plot selection. Exact tested source **`9cfc7ff59aa6fd35a88a2b72e9b5ba01491a3850`** completed the local x86_64 normal-product journey: 24 steps and 24 original PNG checkpoints, with three complete serialize → validate → pack sequences and no terminal kernel refusal. This documentation successor preserves that source identity; it does not relabel the execution as a test of its own commit.

## Retained failure

[Candidate 39684672200, x86_64 job](https://github.com/dancxjo/conduit/actions/runs/39684672200/job/119516116334) failed on tested merge `7a6294a06bc81b14673e8aedb7721b0b48a794b8`. [Integration 39686810134, x86_64 job](https://github.com/dancxjo/conduit/actions/runs/39686810134/job/119523587773) reproduced the failure on `3672758cf3f8bbaec01dab14476882433d31621f`. Both original packets are retained under [negative evidence](evidence/staged-graph-reply-20261021/negative/): build identity, compressed original serial, and compressed original QMP log.

Keyboard Canvas had already accepted `A`. The next selection toward Memory Lantern wrapped through Patchbay and queried its selected graph. Application sequence 88 refused `Commit` with `Fault(WorkExhausted)` and one preemption, then reported `operation=graph slot=Some(0)` and terminal `native-body-kernel-boundary-refused`. The collector subsequently reported a selection timeout. These were execution failures, not evidence that the observation window needed enlargement.

## Repair and measured result

`Commit` now retains the selected Graph query; separate ordered requests serialize a borrowed graph, validate the immutable framing, then pack it in turns consuming at most 4 KiB of decoded input. No deep graph clone, entry-budget increase, storage-grant increase, retry, or acceptance bypass is added. Pending state rejects interleaving, skipped or duplicate phases, premature reads, wrong packing offsets, and replacement of unread output. Root still decodes the existing typed Graph reply and checks graph equality.

The narrow selection collector checks complete typed terminal kernel signs before accepting a revision. It preserves the failure packet and actual target Plot in its diagnostic. Partial tails and recoverable checked-action refusals, including unavailable Wake, remain distinct.

Retained [test logs](evidence/staged-graph-reply-20261021/tests/) show 36 Application-core tests and five collector tests passing. The production bitmap allocator regression passed repeated large Graph/projection exchanges with peak **2,040,832 bytes** in the unchanged **2,072,576-byte** private grant, leaving **31,744 bytes** of rounded live headroom.

The [journey receipt](evidence/staged-graph-reply-20261021/journey-proof.json), [frame manifest](evidence/staged-graph-reply-20261021/journey-frames/manifest.json), [raw serial](evidence/staged-graph-reply-20261021/journey-serial.log.gz), and [raw QMP](evidence/staged-graph-reply-20261021/journey-qmp.log.gz) bind the completed journey to its original source, build, image, Host, Boot, and Body. The manifest reports `complete`, no failure, and 24 checkpoints. All 24 PNGs retain their original bytes and manifest hashes. The [observation summary](evidence/staged-graph-reply-20261021/observation-summary.json) records three begins and three completions for each reply phase.

## Proof boundary and integrity

This is one local freestanding-emulator journey using real QMP keyboard/pointer input and the normal `q35-single-cpu-512m-normal-product` profile. Serialization and validation each remain a full admitted pass; this execution establishes their fit for the recorded workload, not every admitted graph. Incremental compression retains internal buffered work. The allocator regression establishes the recorded fixed-grant workload, not arbitrary capacity headroom.

At this evidence capture, the enclosing supported graphical-profile command was still building/checking four headless asset-closure images. Its complete result is not claimed here. Current-source Candidate, exact-head Integration, reviewed merge, stable publication, physical execution, human enactment, and issue closure are not supplied by this packet. The earlier failing hosted runs remain negative evidence.

[SHA256SUMS](evidence/staged-graph-reply-20261021/SHA256SUMS) covers every packaged file except itself. [Raw-log hashes](evidence/staged-graph-reply-20261021/raw-log-sha256.json) record uncompressed byte lengths and SHA-256 values; each gzip stream was decompressed and compared byte-for-byte with its input. Receipt, manifest, and PNG copies were checked against the preserved execution packet. Build/image JSON records retain ELF/ISO identities; ELF and ISO payloads are not included.
