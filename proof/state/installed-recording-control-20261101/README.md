# Installed recorder control entrance qualification

Source pins, exact terminal checks and ignored log custody are recorded below. The new installed entrance uses the existing local authenticated Request/Response channel, existing BodyRunRequested preparation response and the original General worker. It accepts only a closed operation, finite run/preparation limits, exact expected Body/Boot, and independently selected authority for snapshot effects. It accepts no arbitrary source, resource reference, storage path or receipt. State-only recovery accepts no authority argument. Existing token, busy/native IO, attachment and selected-custody guards apply; compile-unavailable profiles explicitly refuse.

The runtime keeps selected environmental equipment resident and gives the ordinary General worker the existing Owner Host. Preparation may change selected generation before refusing; the runtime reconciles the actual observable offer-generation marker before returning the error. A successfully created worker is retained before fallible marker IO, so marker failure cannot drop a live worker or falsely imply rollback. No retry is introduced.

One shared strict seven-field decoder handles both selected authority files and nested authority on the control wire; Core AuthorityGrant schema is unchanged. CLI reads a regular nonempty JSON file bounded to16KiB, requires authority for snapshot operations and forbids it for State-only recovery. Unknown/duplicate fields and oversized selected grant input refuse. Empty-struct State-only operation closes serde's unit-variant extra-field acceptance while preserving the wire tag.

The actual owner fixture moves the original Host into the service Runtime and back into the same Owner without a replacement advertisement/ledger. Authenticated handle dispatch performs all six operations through actual General workers: publication, independently authorized readback, catalog checkpoint, State discovery, recovered catalog read and recovered chunk read. It drives actual worker progress and lifecycle persistence, checks final admitted read completion, preserves pending producer prefix and explicitly denies producer retirement. It checks wrong Body/Boot/limits before generation changes, busy-worker refusal, and a publication grant refused for readback after generation selection with the observable marker still reconciled. Missing/wrong tokens refuse before preparation.

This is in-process authenticated production dispatcher/worker plus CLI parser/file-decoder proof using a synthetic accepted codec prefix. It is not a live Unix socket/installed daemon/physical Source run or prior sensing proof. Existing native IO guard behavior remains in production, but this fixture carries no live physical equipment. No ACK, producer retirement, whole-Root allocator custody or physical power-loss survival is claimed. BodyRunRequested means preparation/start requested, not committed or read back.

Shared qualification environment, exclusively owned existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

Final strict wire/CLI/actual dispatcher run at `c0e06200879d52a32ab50dce88649cbe124a06d4` passed **6/6**, 401 filtered; 11.25s build / 23.52s tests:

```sh
cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit body_recording -- --nocapture
```

Initial `1b09fdd469` run compiled and passed actual six-operation dispatcher, CLI2 and authentication tests, but failed the strict State-only schema negative (4 passed/1 failed). Its ignored log preserves the serde unit-variant hole. Empty-struct correction at `97aff5fe79` plus CLI constructor `dab1d260e1` then passed5/5 (11.98s build / 22.53s tests), before the independently justified shared nested grant decoder/refusals were added for the final run.

The default-feature exact run at `c0e0620087` passed **6/6**, 360 filtered; 1m28s build / 0.09s tests:

```sh
cargo +stable test -p conduit --bin conduit body_recording -- --nocapture
```

This includes explicit unsupported-profile refusal without Host mutation. It excludes the installed worker fixture, includes the same strict common request/wire/file and CLI tests, and reported one unused current-Body getter warning in this compile-unavailable profile.

After integrating current dev, exact merged source `14d4de02c57e48da9dcfedb848b06b19ccc679b5` passed the supported-profile command above again: **6/6**, 401 filtered; 35.57s build / 25.22s tests, no warnings. Premerge default and supported results remain pinned to `c0e0620087`; they are not relabeled as merged-source runs. The merged source includes an exact cfg on the current-Body getter to avoid exposing unused installed-only support in the unavailable default profile.

The independently requested owning browser closure check on exact merged `14d4de02c5` passed **5/5**, 10 filtered; 2m03s build / 7.19s tests, no warnings:

```sh
cargo +stable test -p conduit-browser-mask-offer --lib owner_component -- --nocapture
```

It verifies finite descriptor/complete pinned graph, reviewed dependency closure, stale/foreign admission refusal, unchanged component Fore contract and exact required-item roots. This is source/descriptor proof, not live browser/WASM or device deployment proof.

Required `cargo +stable xtask ci pipeline evidence-retention` passed exit0 on merged source. All qualification Cargo handles are terminal and the shared native cache is released. Root's independent release build uses another cache and remains Root-owned. No raw logs were staged or committed.
