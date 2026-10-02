# Canonical reviewed plots

This directory owns product-facing authored plot source. Each canonical plot
has one stable `plots/<name>/main.conduit` owner whether it is used as a workload
root or recursively behind another plot's front. Tour, Crèche, Patchbay, CLI
workflows, bodies, and conformance consume those same bytes; they do not own
editable copies.

`proof/fixtures/plots/` remains the explicit home for historical, malformed,
or proof-only specimens. Directory presence alone never promotes such input
into the reviewed inventory.

Adding or changing a canonical plot requires checker coverage and explicit
consumer updates. plot source grants no host, membership, authority, plan, or
play truth.

Conduitese plots remain alive when their admitted work drains unless the closing
brace carries a trailing full stop (`}.`). The full stop means that draining
fulfills the plot's authored meaning; it is not a Gear, effect, or host instruction.
There is deliberately no `complete` keyword or compatibility spelling.

An exact closed structured variant can be routed as an exhaustive railway
switch:

```conduit
event >> ? {
    [MusicEvent.note] >> . >> play-note
    [MusicEvent.rest] >> . >> keep-silence
}
```

`?` selects exactly one track. Every declared case must appear exactly once;
gaps, duplicates, mixed variant types, and unreachable tracks are refused
before planning. Each track lowers to an ordinary typed selector gear and Cord,
so the immutable expanded Plot exposes every possible track while only the
selected track receives a value. Bare `_` is reserved for the final otherwise
track of an open predicate switch; a closed variant must remain explicit. On
the right of a pattern, `_` names the value carried into that track.

[Startup Chime](startup-chime/README.md) and
[First wake Chime](first-wake-chime/README.md) demonstrate non-graphical
embodiment: a body wakes its installed plots, and a plot may simply make a
sound. The first is an optional browser default; the second demonstrates the
reusable body-scoped first-wake source. Both use the same portable sound kind.

`inventory.toml` is the bounded reviewed-membership registry. `cargo xtask
check plots check` validates every declared entry and ratchets every canonical
`plots/<name>/main.conduit` owner into the registry; it never promotes arbitrary
source by scanning for `.conduit` files. `cargo xtask check plots report` emits the
machine-readable per-plot result seam. Gated execution remains `unavailable`
until its deterministic, browser, device, or physical owner supplies evidence.

Run the declared execution oracles with:

```sh
cargo xtask check plots run --deterministic
cargo xtask check plots run --browser
```

Deterministic execution continues through individual plot failures. Browser
execution builds the WASM runtime and runs reviewed browser-safe cases with
pinned Chromium, one worker, and zero retries. It needs the repository's
Playwright installation; absent prerequisites are reported as unavailable.
These cases do not acquire devices or grant browser permissions.

`cargo xtask check plots report` executes deterministic declarations and includes
availability for gated proofs. Add `--dry-run` to inspect planned work without
running execution oracles. The report distinguishes failed, unavailable,
not-applicable, and refused results.
