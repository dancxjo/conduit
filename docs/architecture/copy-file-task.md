# Copy-file task front readiness

This is the first M5 slice for a task-facing copy-file workflow.

The task front is deliberately narrower than a Workbench or file manager. It
lets a user choose one source path and one destination path, then runs a
generated form through the normal host protocol. The chosen paths are task
resource bindings; they are not embedded into the generated form source.

## User-facing command

```text
conduit copy-file --source <path> --destination <path> [--replace|--reject-existing] [--max-bytes <n>] [--inspect]
```

The operator output names:

- the request and run IDs;
- the source and destination choices;
- the protected source and destination binding IDs;
- the preflight decision, such as `will-create`, `will-replace`, or
  `reject-destination-exists`;
- the plain-language result;
- a receipt tying request, run, plan, source binding, destination binding, and
  result together.

## Generated form boundary

The generated form is path-free:

```text
form 0

copy_file_task {
 copy: task/copy-file
 record: task/copy-file-receipt
 copy.receipt -> record.in
}
```

The source and destination paths live in `CopyFileRequest` resource bindings.
The hosted implementation receives those bindings from the task front, not from
form configuration. Inspect output can reveal the generated form and exact plan
after the user-facing task works.

## Current result distinctions

The first slice distinguishes:

- created;
- replaced;
- destination already exists and was rejected before runtime;
- stale source handle or missing destination parent;
- permission denial;
- oversized input;
- cleanup failure after a copy error;
- generic failure.

The hosted copy operation emits a bounded copy receipt value to a receipt
operation, so the plan contains ordinary placement, connection, and terminal
evidence.

## Stop-line

This does not close M5 by itself. Remaining M5 work includes a browser-safe
chooser/create-target experience, a real Stop/cancellation control during a
long-running copy, partial-copy receipts, and unfamiliar-user observation.

## Checkpoint commands

```text
cargo test -p conduit-std-host
cargo run -p conduit -- copy-file --source <path> --destination <path> --inspect
just check-copy-task-readiness
```
