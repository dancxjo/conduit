# S3 lossless form checkpoint

This checkpoint adds an editing document around the existing deliberately small
`form 0` checker. It restores only the archive seam that remains useful:

- the exact UTF-8 source text;
- bounded whitespace, comment, and lexeme CST tokens whose concatenation is the
  source byte-for-byte;
- exact UTF-8 byte extents with one-based line and character columns; and
- stable located diagnostics while retaining all source after an error.

`parse_document` keeps editable syntax and checked meaning separate. A valid
document contains the same `CheckedForm` produced by the compatibility `parse`
entry point. An invalid document retains its source and tokens but has no
`CheckedForm`; downstream planning therefore cannot execute recovered or
partially understood syntax.

Source text is limited to 1 MiB and the CST to 131,072 tokens before semantic
checking. An oversized source is rejected before it is copied into the
document. Limit failures are explicit diagnostics and do not fall back to an
unbounded or partial executable representation.

## Identity boundary

Layout and comments change `SourceDocumentId` but do not change
`CheckedFormId` or `ExpandedFormId`. A semantic edit changes all downstream
semantic identities. Plan, play, evidence, and presentation identities are not
conflated with this document and remain work for their own salvage checkpoints.

## Deliberate exclusions

This is not a transplant of the archived panel grammar. It does not restore
modules, packages, a formatter, UI recovery nodes, or panel-era compatibility.
The current one-cell syntax and its export rule are unchanged by the lossless
document layer. Inline nesting remains later S3 work.

## Checked composite boundary checkpoint

`CheckedForm::export_boundary` is the sole conversion from an authored export
to an externally consumable composite contract. It binds the authored
capability and kind, the checked form identity as the contract revision, the
exact internal source/sink endpoints, and the checked external ports. Missing
or duplicate capability exports fail closed.

Both `CompositeDefinition::from_authored_export` and
`ProfileCatalog::insert_export` consume this same checked object. The composite
helper no longer fabricates a `kind@1` revision, and parent helper code cannot
declare a boundary absent from the child source. A parent operation created
from the installed export uses the normal checker and planner cord path; it
does not address internal child identities.

## Inline nesting checkpoint

An inline child uses `operation: capability { ... }`. The inner block is
checked recursively as the same `CheckedForm` used for a standalone document.
The named capability must be an explicit checked export; that boundary becomes
the parent operation's exact kind revision and ports. Parent connections then
use the ordinary explicit or single-port shorthand checker.

Nesting is limited to 16 levels. Inner diagnostics retain exact outer-document
spans and all later tokens. A standalone child and the same inline child have
different source-document identities but identical checked, expanded, and
export-boundary identities. The parent retains the checked child rather than a
parallel recovery AST.

## Runtime identity checkpoint

The identity chain does not stop at expansion:

- `PlanId` identifies one immutable exact plan;
- `ActivePlayId` identifies one activation, bound to plan, host, boot, and a
  monotonic host activation sequence;
- `EvidenceId` identifies one host-recorded observation, bound to host, boot,
  optional active play, and a monotonic evidence sequence; and
- `PresentationId` identifies one presentation request, bound to active play,
  placement, and a monotonic per-placement sequence.

Presentation effects carry both play and presentation IDs through std,
browser-shaped, Pico-shaped, and composite adapters. A completion with either
the wrong play or presentation identity is rejected without consuming the
pending request. Observatory uses the runtime-issued evidence ID and projects
its play/presentation references; it no longer fabricates `evidence/{row}`
identities.
