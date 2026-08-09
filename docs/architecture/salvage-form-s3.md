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
semantic identities. Hidden nested implementation changes leave the parent's
checked identity alone when its visible exported contract is unchanged, but
change the parent expanded identity and therefore its plan/fragment identities.
Plan, play, Sign, and presentation identities are not conflated with this
document.

## Deliberate exclusions

This is not a transplant of the archived panel grammar. It does not restore
modules, packages, a formatter, UI recovery nodes, or panel-era compatibility.
The current one-gear syntax and its export rule are unchanged by the lossless
document layer. Inline nesting remains later S3 work.

## Checked composite boundary checkpoint

`CheckedForm::export_boundary` is the sole conversion from an authored export
to an externally consumable composite contract. An export is a block of zero
or more named input faces and zero or more named output faces. Each input maps
to one exact checked internal sink endpoint; each output maps to one exact
checked internal source endpoint. Direction, value kind, and the currently
supported `independent` terminal contract are explicit and identity-bound. No
face is derived by finding an internal connection.

The visible contract revision binds only external face names, directions,
value kinds, and terminal contracts. Internal operation/port identities remain
in the checked mapping and are deliberately absent from the parent-visible
kind. Duplicate face or capability names, wrong directions/kinds, missing
endpoints, and unsupported terminal policies fail closed.

Both `CompositeDefinition::from_authored_export` and
`ProfileCatalog::insert_export` consume this same checked object. The composite
helper no longer fabricates a `kind@1` revision, and parent helper code cannot
declare a boundary absent from the child source. A parent operation created
from the installed export uses the normal checker and planner cord path; it
does not address internal child identities. The checker covers
multi-input/multi-output faces with different value kinds plus input-only and
output-only composites.

The hosted composite compatibility façade configures those checked mappings
on each exact child fragment before Play start. Parent connection envelopes
enter only the mapped child input port; atomic named child outputs leave only
the matching parent face. Item/byte pressure retains and retries the same
envelope, each input/output closes independently, and cancellation or child
failure terminalizes every visible connection before parent plan Sign.
External events and observations use only parent plan/connection identities;
child host and placement identities remain internal. This does not restore the
legacy runtime as the production engine: `conduit-std-host` continues to run
its installed profiles through `conduit-kernel`.

## Inline nesting checkpoint

An inline child is one configured Gear whose Kind is a reusable Form. The inner block is
checked recursively as the same `CheckedForm` used for a standalone document.
The named capability must be an explicit checked export; that boundary becomes
the parent operation's exact kind revision and ports. Parent connections then
use the ordinary explicit or single-port shorthand checker.

Nesting is limited to 16 levels. Inner diagnostics retain exact outer-document
spans and all later tokens. A standalone child and the same inline child have
different source-document identities but identical checked, expanded, and
export-boundary identities. The parent retains the checked child rather than a
parallel recovery AST.

## Nested expansion identity correction

Each parent expanded identity now binds a canonical row for every nested
Gear: its Gear path, selected export capability, and the child's
recursively expanded identity. Rows are sorted by Gear identity, so source
declaration order is spelling rather than semantics; swapping implementations
between two paths still changes the expanded identity.

`CheckedForm::validate_identities` recursively recomputes checked and expanded
identities and checks each nested operation against its selected export.
Planning invokes this validator before placement or resource work. Omitting,
duplicating, reordering, or substituting a nested row while retaining a sealed
identity therefore fails closed before a plan can be issued.

## Runtime identity checkpoint

The identity chain does not stop at expansion:

- `PlanId` identifies one immutable exact plan;
- `ActivePlayId` identifies one Play start, bound to plan, host, boot, and a
  monotonic host Play start sequence;
- `SignId` identifies one host-recorded observation, bound to host, boot,
  optional active play, and a monotonic Sign sequence; and
- `PresentationId` identifies one presentation request, bound to active play,
  placement, and a monotonic per-placement sequence.

Presentation effects carry both play and presentation IDs through std,
browser-shaped, Pico-shaped, and composite adapters. A completion with either
the wrong play or presentation identity is rejected without consuming the
pending request. Observatory uses the runtime-issued Sign ID and projects
its play/presentation references; it no longer fabricates `sign/{row}`
identities.
