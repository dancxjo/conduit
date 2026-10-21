# Pinned native typography mechanism (#5992)

The bounded serif/sans/mono mechanism from merged #6165 is verified on dev
`6a8e127b48c66b04ec0801cc4778c8762f36de72`. This completes #5992's expressly
permitted mechanism-first slice. Actual selection of two Looks and preservation
of focus/accessibility remain #5628/#5629, the next steps of #5994. No separate
Look engine or Face font-family state is introduced.

## Current product and exact source

Both fresh captures began with clean committed source and finished with the
same clean source. Their complete commands, statuses and logs are retained.
The graphical entrance was:

```
cargo xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output PATH --locked
cargo xtask make host verify PATH --journey --locked
```

It builds the canonical PROFILE-owned **product-host**, not an architecture
proof appliance. The final image SHA-256 is
`17f97a46070b180e416c90eabe60e2a29366446be083ed7aeb76df1adbf92d10`.
The guest's resolved-description binding is separately
`image:sha256:093b4370193d1862c298b9b11f48dfc6eea9055ffffffae33dbdef02777e976b`;
it is not the final packaged image digest. The source, PROFILE, BUILD, image,
Host and fresh Boot are bound by the ordinary product journey gate.

All **22 steps** pass, retaining **22 actual QEMU PNG captures**, source-driven
4→3→4 choices by keyboard/pointer, Birth/Wake/Plan/Play/Stop, Home/Body/Patchbay,
protected ordinary execution and failure/refusal transitions. The real
[initial Crèche](evidence/native-typography-acceptance-20261020/graphical/x86_64/journey-frames/front-door-ready.png)
shows the serif source-owned title and sans controls; this and the native
Details capture were visually inspected. The [complete journey receipt](evidence/native-typography-acceptance-20261020/graphical/x86_64/x86_64-product-journey-proof.json)
and compressed serial evidence retain the exact typed Show/interaction custody.
This is freestanding-emulator evidence, not physical/human enactment, a browser
comparison or completion of the broader graphical parent.

The separate headless PROFILE was built into a different retained directory:

```
cargo xtask make host build targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit --output PATH --locked
cargo xtask make conduitos headless-proof PATH --locked
```

The final headless image SHA-256 is
`6c28c992615ee712cc48bc6ae828f5762501fc66260e00b93a73e59929b6f2a0`.
Final ELF inspection excludes graphical symbols and all three pinned DejaVu
assets. Two actual boots have distinct Host/Boot identities and the explicit
unsupported-startup outcome; no initialized workload is claimed. The
[headless receipt](evidence/native-typography-acceptance-20261020/headless/image/headless-proof.json)
retains the exact observations. Two existing headless binary warnings (unused
`identities` and unread `InspectedSpore` fields) remain visible in the complete
build log; no strict headless-binary warning-free claim is made.

## Mechanism, bounds and retained earlier proof

Current dev passes **20 actual typography tests**, with 1,209 unrelated tests
filtered, none ignored or failed. They check selected/fallback/rescue ink at
100–200% scale, shared measure/wrap/caret/paint geometry, exact Unicode byte
admission, zero-advance marks, narrow refusal before paint, proportional prose,
monospace code and immutable table bounds. This does not claim general shaping.

The pinned source fonts and complete retained license match their declared
hashes. Current generated tables, coverage and resource ledger exactly match
all three hashes of the earlier source-pinned mechanism proof. Its ensemble
contains 1,000 admitted scalars and **880,471 bytes** of coverage/records/profile
resources; existing 4 MiB coverage, 1,024-scalar, 64-pixel glyph, 48-command,
192-byte command-text and eight-surface limits stay unchanged. Font parsing is
make-time only; raster lookup consumes immutable tables.

Earlier narrow/wide Unicode/editing/pointer and eight-/21-page typed Details
proof, the before/after comparison, mono technical specimens and allocation
controls retain original source `6f579f684500e6bf54c5dd7b5870c5c5c6fb7d6d` and
[immutable provenance](https://github.com/dancxjo/conduit/blob/6391be2de4ae5ee7e693833135585b311799babb/task-6038/outputs/5992-6f579f684/receipt.json).
The changed mechanism production files have identical Git blobs on current dev;
Tour's only difference is test extraction, with identical production prefix.
Those old captures are not relabeled as new current-source runs. Their
allocation boundary is 640 text renders on a stack surface, not the entire
`NativeFaceScene::frame`; static ensemble bytes exclude rescue resources, code
and peak memory. The retained ledger keeps those quantities separate.

[Hashed complete evidence](evidence/native-typography-acceptance-20261020/manifest.json)
includes all current captures, statuses, image descriptions, full logs and older
source-bound records. Final ELF/ISO bytes remain locally retained with their
hashes and reproducible commands; this evidence directory is not a published
release bundle. Known pagination issue #6158 is excluded by the user and is not
claimed fixed. The [browser repair](chromium-single-worker-20261020.md) has actual
complete Candidate proof; the user's CI bypass applies to combined Integration.
No stable publication is claimed.
