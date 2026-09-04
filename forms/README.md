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

`inventory.toml` is the bounded reviewed-membership registry. `cargo xtask
forms check` validates every declared entry and ratchets every canonical
`forms/<name>/main.conduit` owner into the registry; it never promotes arbitrary
source by scanning for `.conduit` files. `cargo xtask forms report` emits the
machine-readable per-Form result seam. Gated execution remains `unavailable`
until its deterministic, browser, device, or physical owner supplies evidence.

`cargo xtask forms run --deterministic` executes every inventory-declared
deterministic oracle in fresh process state and continues through individual
failures. `cargo xtask forms run --browser` reports the still-unconnected
browser proof seam without prompting for permission or acquiring a device.
`forms report` executes the deterministic declarations before emitting its
aggregate report; `--dry-run` lists them as unavailable planned work.
