# Checked Root startup inputs

A Host can prepare an independently selected typed value for a Root startup
parameter before canonical expansion. This supports presentation configuration
such as the Look intent required by #6461 without editing Source or extracting
the desired value from an already expanded Plot or Plan.

`PreparedRootStartupInputs::prepare` binds the values to the exact checked
document and public Root. It accepts at most 64 named inputs, with a combined
canonical extent bounded by `MAXIMUM_STRUCTURED_CANONICAL_BYTES`. It checks
declared Types, startup byte contracts, native field contracts and invariants.
Unknown names, duplicates, foreign Types, unresolved values and excess capacity
refuse before expansion. Omitted parameters retain their checked defaults;
omitted required parameters still refuse during expansion.

`expand_canonical_plot_for_authoring_with_startup` consumes this sealed
preparation. Another Root or changed checked document cannot reuse it. Selected
values affect expansion configuration and identity while Source and checked
Plot identity remain unchanged. Existing expansion entrances retain their
default behavior.

Run the focused proof through:

```sh
cargo +stable xtask prove environment history-root-startup
```

This checks canonical expansion regressions, strict owning-package lint and the
wasm32 library. It establishes checked configuration custody, not storage
commit, membership, Host resource admission, live recording or a displayed Show.
