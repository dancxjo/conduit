# Recorder snapshot publication

`RecordingSnapshotBack` validates one prepared, fixed-cardinality recording chunk
and delegates its publication to the existing kernel `HostCallBack`. Root still
selects and admits the actual snapshot implementation, resource, placement,
authority, queues and complete work/storage closure before starting a Play.
Preparation and matching identity bytes do not create that authority.

The returned `RecordingCommitObserver` belongs to this Back. It exposes the exact
selected generation reference as identity data; its recorded sequence range stays
absent until the actual Host Call completes with those exact bytes and the kernel
commits the Back's output step. Output pressure and an uncommitted or cancelled
step cannot advance it. A successful storage service completion alone is not the
kernel commit. A later cancellation preserves a range already committed.

Malformed chunk content, wrong context/cardinality, mismatched reference bytes,
missing output and invalid lifecycle refuse explicitly. Host Call failure, denial
and cancellation remain their original terminal dispositions. This Back admits
one immutable generation; it does not invent retries, replacement generations,
a scheduler, storage provider, catalog update or an acquisition timestamp.

`cargo xtask prove environment recording` includes deterministic tests of the
actual recorder Back and production kernel. Placement and storage completion are
explicit CPU fixtures. Success, storage failure, denial, cancellation, corrupt
content and a foreign completion reference all reach checked terminal outcomes.
The recurring kernel path allocates no Rust heap. These tests do not establish
actual file/browser durability, live capability admission, a native producer Line,
Body membership, Pi boot or overnight operation. Those integrations remain part
of the continuing recorder appliance work (#6295).
