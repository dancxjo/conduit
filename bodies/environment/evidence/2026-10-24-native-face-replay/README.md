# Native Face replay verifier

Source commit 2a31fafee adds an independent producer Face replay comparison to
the retained native owner. The supported face-publication proof passes, including
two replay boundary tests; strict scoped native-owner and replay-test lint passes.
The device reply uses one observation instant for the signed raw snapshot and
Face freshness. The optional owner-face-run-esp32 command requires replay
comparison during the finite native run.

The attempted physical run was interrupted before flashing: an erroneous
`cargo clean --target wasm32-unknown-unknown --target-dir target` removed the
entire local target tree rather than only the intended inactive platform cache.
This deleted build caches and protected ignored ELF/full-capture artifacts,
including the protected operational ELF and the newly retained native Face map.
Committed source and curated evidence remain, and private keys were outside
target. Historical manifest hashes identify deleted artifacts; those full
ignored artifacts are no longer available locally. Do not claim otherwise.

Read-only serial inspection after interruption found six current available
bench updates with successful physical display writes; the last still had
sequence 101 and a running clock over 26 minutes. This contradicts a diagnostic
flash having occurred. There is no new physical native Face proof. A full flash
backup and subsequent fresh bench verification are recorded separately. Future
diagnostics must keep their recovery image and unique captures outside every
Cargo target directory. Do not run non-package Cargo clean against a shared
output directory, even with --target.

The replacement 4 MiB full-flash backup is private, mode 0600, under the host
Conduit data directory outside Cargo targets. A flushed UART capture after its
read/reset verifies twelve current acknowledged display updates across four
fresh source sequences (1–4). The recovery receipt pins the backup hash without
publishing firmware contents. This is operational bench verification only.
