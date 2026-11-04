# Conditional Body workset proposals

[#6448](https://github.com/dancxjo/conduit/issues/6448) owns this pure proposal
step. The authored Plots construct intent. They do not install Source, change
membership or a workset, create authority, or start a Plan/Play. Adoption is the
separate guarded owner operation in
[#6449](https://github.com/dancxjo/conduit/issues/6449).

`propose.conduit` imports the checked snapshot/context/label/digest Types from
`semantics/language/source_check_info.conduit`. An authoring context links their
actual checked Native owners under `language/source-check/{snapshot,context,label,digest}`;
matching names alone do not supply those owners. The conformance fixture uses
`insert_checked_native_type_from_document`, not a substitute public schema.

The first authored compositions accept one observed retained Plot and one
checked candidate:

- `propose-add` proposes both retained and candidate Plots.
- `propose-replace` proposes the candidate alone.
- `propose-retain-one` proposes retaining one Plot from a later observed revision.

Every result contains the exact expected Body/workload revision and complete
desired workset. Add/replace preserve Source owner, generation, content hash,
Source identity, entry, checked identity and complete checking-context revision.
Removal explicitly carries no new Source candidate. These values are conditional
when their input observations are historical. A proposal does not assert that
its successor is already current or that checking grants execution permission.

The Rust reference in `make/body/src/workset_proposal.rs` correlates a check
report with independently selected snapshot/context revisions, then reuses
`BodyWorkset` for ordering, duplicates, item limits and aggregate identity bounds.
A complete proposal may contain retained Plots and at most one new checked
candidate. Unknown additional Plots, mismatched identity pairs, malformed reports
and no-op revisions refuse. Source reports and their labels remain descriptive,
forgeable Info; this code does not turn them into compiler custody or a grant.
The authorized adopter must independently obtain/recheck actual Source and current
Body/catalog facts. No compiler AST or checking diagnostics are copied into the
proposal. The projection has a 32 KiB JSON bound in addition to canonical workset
bounds.

The portable Native Info carries a finite sequence of Plot identity pairs. The
Rust descriptive JSON projection retains the existing `BodyWorkset` representation
and an optional Source basis. JSON is not the canonical Native byte codec. Tests
evaluate the actual authored expressions to canonical structured Info and compare
their complete workset and provenance with the reference projection.

`propose_admitted_workset` is the production bridge from canonical Native Info to
that conditional projection. It requires the existing `AdmittedNativeCanonical`
witness from the caller's independently pinned checked declaration. Private
Native-owner correlation uses Conduit's existing prepared-decoder lookup; public
inspection metadata cannot replace it. A different declaration owner with the
same public IDs, a different Native type, absent or changed Source basis, wrong
Body, stale revision, or an oversized canonical value refuses. Only then are the
complete workset and candidate provenance normalized through the shared rules.
This witness proves Native shape/laws and custody, not installation permission.

This is authoring-host preparation. It installs no new MCU runtime, special
thermometer Gear, planner, execution engine or control protocol. The tests prove
checked Source and CPU evaluation, not installed Body adoption or device effects.

Internal repository validation:

```sh
cargo +stable test -p conduit-body-make --test workset_proposal --locked
cargo +stable clippy -p conduit-body-make --all-targets --locked -- -D warnings
```
