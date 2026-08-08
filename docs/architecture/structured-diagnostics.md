# Structured diagnostics v1

Conduit has one narrow, versioned public diagnostic record for the current form-to-plan path. It is not a replacement for every internal error type.

`StructuredDiagnosticV1` carries a stable code, severity, bounded summary, source-document identity, optional content hash and primary span, bounded related subjects, bounded public and redacted argument names, and bounded notes. Human and JSON output render the same owned record. Human wording can therefore improve without changing its stable code or structured meaning.

The form adapter preserves existing `CND-FRM-*` codes and exact parser spans. `conduit diagnose-form <form-file> [--json]` is the executable source-facing path. Paths are used only to read input and are not included in the record.

The planner adapter deliberately recognizes only a reviewed subset of failures. Its capability diagnostic describes the absence of a face-compatible realization; nominal identity is not the compatibility gate. Host-local planner detail is named as redacted and is never copied into the public record. Unreviewed planner failures produce no structured record instead of being collapsed into a generic code.

All collections have fixed item limits and every text field has a fixed byte limit. Oversized form messages are truncated on a UTF-8 boundary with an explicit note. Other invalid records fail validation.

Later work may add adapters for lowering, kernel preparation, host operations, wire sessions, or physical tooling. Those adapters must preserve their own identities and proof classes; this schema does not authorize conflating their failures or exposing private platform facts.
