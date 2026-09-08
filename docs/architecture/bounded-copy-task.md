# Bounded Copy-a-file execution

The hosted Copy-a-file operation has no authored path or resource token. Its `file/copy` Gear means “copy a file” and emits a bounded structured result to an ordinary presentation sink. Planning selects one exact std implementation and binds two protected resource roles from base-issued inputs: a read-existing source and a create-only or replace-existing destination. The Plan contains only opaque handle identities and exact host, boot, capability, resource class, access, byte, and commit bounds.

Execution lowers that Plan through the ordinary numeric kernel boundary. A one-byte command value requests the single admitted `conduit.host/file-copy-step@1` operation. The std base resolves both handles against its private path registry, reads at most 4 KiB, writes at most that chunk to a same-directory temporary file, and returns the same command value when another step is required. Completion produces one structured result on the exact output Port, then the semantic operation completes. The command is therefore finite and kernel-owned without allocating one value per chunk.

Create-only commit uses a same-directory hard link so a concurrent destination cannot be overwritten. Replacement uses a same-directory rename. Neither policy writes the final destination incrementally. Stop is checked between every admitted chunk; it calls kernel cancellation and removes the temporary file. Partial I/O and cleanup failure remain separate results.

Every run returns a receipt naming the request, active Play, Plan, source handle, destination handle, structured result, copied byte count where applicable, and kernel Sign count. Raw paths remain base-private and are absent from the Form, Plan, receipt, and kernel protocol.

The current std profile admits at most 16 MiB per run. The product entrance is `conduit copy SOURCE DESTINATION`; its positional paths become private protected-resource choices, not authored Form facts. Native Patchbay also has a [protected file chooser](native-file-base.md). This hosted contract alone establishes neither a browser copy realization nor a fresh human usability observation.

Linux targets may instead explicitly install the `isolated-file-base` feature
and `conduit-isolated-copy-base` binary. That realization preserves this Form,
Kind, planner, kernel, commit, cancellation, result, and presentation contract;
only the selected implementation and effect boundary differ. Minimal Host core
does not include the provider, and provider failure returns a terminal failure
without falling back to the cooperative implementation.
