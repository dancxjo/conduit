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
Nested forms, checked composite boundaries, exports-derived offers, and normal
parent consumption of child exports remain the next S3 work. The current
one-cell syntax and its export rule are unchanged.
