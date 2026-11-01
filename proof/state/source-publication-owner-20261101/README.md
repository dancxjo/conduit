# Ordinary owner Source chunk publication qualification

Qualified combined source: `c39e905943e4c435e2781d874e3516421d1ef3d5`. Helper: `8480976c2b`, original worker wiring: `942155e66e`, canonical snapshot source-loader prerequisite: `c720e82687`. No sidecar Plan or replacement Host advertisement is used by the helper.

Exact command, exclusive existing native cache:

```sh
CARGO_TARGET_DIR=/tmp/conduit-local-native-provider-check CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +stable test -p conduit --features installed-environmental-sources,installed-body-storage --bin conduit source_recording_stage::publication::tests -- --nocapture
```

Final result: exit 0; **2 passed, 0 failed**, 389 filtered; 10.93s compile, 5.84s tests (`tests-corrected.log`). Initial `942155e66e` compile failure is retained in `tests-942155.log`; prepared content is already a canonical byte slice, corrected to `to_vec()` without changing content or codec.

The positive fixture retains a real private selected directory and original StdHost/Owner, supplies an explicitly authorized snapshot grant, and seeds a synthetic accepted codec prefix. The helper selects the exact generation, validates the independently supplied grant against that new selected offer/Host/Boot/capability, preflights the actual partition, and uses ordinary persisted same-Body resident replacement and session proposal. The real service worker performs the original durable start handshake and runs the existing selected-storage Body kernel. Its strict matching publication Fore and clean completion are retained, an actual snapshot file is created, and the original staged observation plus prepared snapshot backing remain held unchanged. The negative fixture proves stale Boot and absent grant cannot produce a file or publication proposal and do not discard the prefix.

This qualifies local publication of prepared codec content through the actual owner worker. It does not prove prior sensing/native admission (the prefix is synthetic), independent admitted readback, catalog State commitment, a durable Recorder ACK, producer retirement, physical power-loss survival, or whole-Root allocation admission. The caller limit is a named input/source volume limit, not a total Rust allocator bound. A refusal after selection or resident replacement can leave that ordinary prepared generation/resident in place; no atomic rollback claim is made. Exact readback and later catalog commitment remain required before consuming the prefix or authorizing retirement.

No broader target build/test was run. Native cache released after final terminal exit zero.
