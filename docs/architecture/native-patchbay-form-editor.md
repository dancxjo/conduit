# Native Patchbay canonical Form editor

Issue #557 adds one revisioned, toolkit-independent editor state to native Patchbay. The UTF-8
`.conduit` source buffer is authoritative. Each accepted edit increments its revision and passes
through `conduit-form`'s lossless parser and canonical syntax checker. A result may be published
only for the current revision, so delayed work cannot restore stale semantics.

The graph is rebuilt from `CheckedSyntaxDocument` plus the parser's exact spans. Its item
identities name forms, face ports, startup values, gears, and cords; graph selection maps back to
the same byte span. Reusable gears remain one collapsed face item such as `hello: greet` until the
user opens `greet`'s back. That back contains authored Conduit gears and cords, never bases,
implementations, plans, or runtime state.

Native Patchbay opens only regular `.conduit` files within the existing finite Form source bound.
Save writes and syncs a same-directory temporary resource before rename. The presentation is also
finite. Invalid source retains the editable text and renders the parser/checker's structured code,
message, and exact line, column, and byte span; it does not manufacture graph semantics.

This slice intentionally has no graph mutation, graph file format, formatter, execution path, or
catalog authority. A later graph edit must first generate canonical source and return through this
same revisioned parse/check path.
