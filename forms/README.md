# Canonical reviewed Forms

This directory owns product-facing authored Form source. Each canonical Form
has one stable `forms/<name>/main.conduit` owner whether it is used as a workload
root or recursively behind another Form's face. Tour, Crèche, Patchbay, CLI
workflows, bodies, and conformance consume those same bytes; they do not own
editable copies.

`proof/fixtures/forms/` remains the explicit home for historical, malformed,
or proof-only specimens. Directory presence alone never promotes such input
into the reviewed inventory.

Adding or changing a canonical Form requires checker coverage and explicit
consumer updates. Form source grants no Host, membership, authority, Plan, or
Play truth.

[Startup Chime](startup-chime/README.md) and
[First Wake Chime](first-wake-chime/README.md) demonstrate non-graphical
embodiment: a Body wakes its installed Forms, and a Form may simply make a
sound. The first is an optional browser default; the second demonstrates the
reusable Body-scoped first-wake source. Both use the same portable sound Kind.

`inventory.toml` is the bounded reviewed-membership registry. `cargo xtask
forms check` validates every declared entry and ratchets every canonical
`forms/<name>/main.conduit` owner into the registry; it never promotes arbitrary
source by scanning for `.conduit` files. `cargo xtask forms report` emits the
machine-readable per-Form result seam. Gated execution remains `unavailable`
until its deterministic, browser, device, or physical owner supplies evidence.

Run the declared execution oracles with:

```sh
cargo xtask forms run --deterministic
cargo xtask forms run --browser
```

Deterministic execution continues through individual Form failures. Browser
execution builds the WASM runtime and runs reviewed browser-safe cases with
pinned Chromium, one worker, and zero retries. It needs the repository's
Playwright installation; absent prerequisites are reported as unavailable.
These cases do not acquire devices or grant browser permissions.

`cargo xtask forms report` executes deterministic declarations and includes
availability for gated proofs. Add `--dry-run` to inspect planned work without
running execution oracles. The report distinguishes failed, unavailable,
not-applicable, and refused results.
