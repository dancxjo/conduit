# Canonical reviewed forms

This directory owns product-facing authored form source. Each canonical form
has one stable `forms/<name>/main.conduit` owner whether it is used as a workload
root or recursively behind another form's front. Tour, Crèche, Patchbay, CLI
workflows, bodies, and conformance consume those same bytes; they do not own
editable copies.

`proof/fixtures/forms/` remains the explicit home for historical, malformed,
or proof-only specimens. Directory presence alone never promotes such input
into the reviewed inventory.

Adding or changing a canonical form requires checker coverage and explicit
consumer updates. form source grants no host, membership, authority, plan, or
play truth.

[Startup Chime](startup-chime/README.md) and
[First wake Chime](first-wake-chime/README.md) demonstrate non-graphical
embodiment: a body wakes its installed forms, and a form may simply make a
sound. The first is an optional browser default; the second demonstrates the
reusable body-scoped first-wake source. Both use the same portable sound kind.

`inventory.toml` is the bounded reviewed-membership registry. `cargo xtask
forms check` validates every declared entry and ratchets every canonical
`forms/<name>/main.conduit` owner into the registry; it never promotes arbitrary
source by scanning for `.conduit` files. `cargo xtask forms report` emits the
machine-readable per-form result seam. Gated execution remains `unavailable`
until its deterministic, browser, device, or physical owner supplies evidence.

Run the declared execution oracles with:

```sh
cargo xtask forms run --deterministic
cargo xtask forms run --browser
```

Deterministic execution continues through individual form failures. Browser
execution builds the WASM runtime and runs reviewed browser-safe cases with
pinned Chromium, one worker, and zero retries. It needs the repository's
Playwright installation; absent prerequisites are reported as unavailable.
These cases do not acquire devices or grant browser permissions.

`cargo xtask forms report` executes deterministic declarations and includes
availability for gated proofs. Add `--dry-run` to inspect planned work without
running execution oracles. The report distinguishes failed, unavailable,
not-applicable, and refused results.
