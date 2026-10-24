# Exact resource-reference equality (#6400)

Run the owning check with:

```sh
cargo +stable xtask check resource-reference-equality
```

A `ResourceRef` is exact portable info. Comparison includes semantic identity,
version, content profile, access class, extent and expiry fields. Core validates
its bounded codec throughout canonical input before structured equality can
short-circuit. Comparison grants no read access and performs no resource lookup.

The check covers allocating and prepared direct/nested equality and inequality,
each expiry field, malformed payloads, skipped record/variant/sequence members,
existing structured equality laws, and prepared evaluation without allocations
or reallocations. The primitive scalar-tape profile remains unsupported.

The installed-kernel case selects the existing Std pure-expression Back and
executes equal and foreign-version references through Source, Plan and the
ordinary kernel. This Back uses its admitted generic Host Call; no resource-read
grant or resource residence is installed. Kernel value capacity remains stable.

The supported check passes 11 cases and the existing Core reference suite passes
8 more, with zero ignored. Strict owning library/test and CLI Clippy, formatting
and patch checks pass. `results.json` binds all nine Source files, four actual
test executables and raw logs to the code commit. Historical negative controls
and corrected fixture failures remain explicitly labelled beside final results.

Candidate and accepted Integration remain separate requirements. This prerequisite does not deliver #6072's adopted lexicon,
resource-backed parser epochs or any model quality claim.
