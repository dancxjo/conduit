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

The deterministic [recorder tests](../runtime/tests/recording_publication.rs)
exercise the actual Back and production kernel. Placement and storage completion are
explicit CPU fixtures. Success, storage failure, denial, cancellation, corrupt
content and a foreign completion reference all reach checked terminal outcomes.
The recurring kernel path allocates no Rust heap. These tests do not establish
actual file/browser durability, live capability admission, a native producer Line,
Body membership, Pi boot or overnight operation. Those integrations remain part
of the continuing recorder appliance work (#6295).

## Durable catalog selection

The authored `catalog-seed.conduit` retains an optional ResourceRef in Body-lived
State. Its read-only companion keeps the same State identity and exact type,
without accepting a replacement value. Residence and authority remain Host/Plan
facts; the reference supplies neither.

`StdHost::run_body_durable_external_plot_to` supplies Root's Body/residence and
the sealed external Fore to the existing installed kernel and synchronized State
file implementation. The metadata recovery result is narrowed to its validated
fixed header before the subsequent value-recovery request; the State-sized result
allowance does not become a larger metadata input grant.

Run `cargo xtask check recording-seed`. Separate child processes write, reopen
under a fresh Boot/Plan, inspect a distinct Body and refuse a corrupt generation.
The exact canonical reference and original published bytes are preserved during
read-only recovery. Host offers and Body admission remain CPU fixtures, and the
catalog reference is fixture data. This proves installed State and actual file
recovery, not prior recording/catalog publication, live membership or an overnight
recorder service. Those connections remain separate integration work (#6358).

## Installed publication checkpoint

`catalog-checkpoint.conduit` connects JSON publication to the same Body-lived
State through the ordinary `some(ResourceRef)` expression. Its Fore accepts only
the JSON payload; the selected publication returns the reference. The prepared
expression evaluator preserves the first-class reference type, validates its Core
encoding without allocation and uses the codec's finite storage bound.

The optional `conduit-std-host` `linux-snapshot` feature installs exact publication
and read Backs. Root supplies a separately retained `SnapshotDirectory` through
`StdHost::run_body_durable_snapshot_plot_to`; constructing a descriptor does not
admit a directory, storage capacity or authority. File effects run outside kernel
Steps and complete the original Host Call. Other execution profiles supply no
directory custody. An occupied generation or failed effect cannot produce a
successful reference for downstream State.

Run `cargo +stable xtask check recording-checkpoint`. The suite checks authored
composition, bounded allocation-free reference expressions, actual file
publication feeding installed durable State, refusal preserving the previous
seed byte for byte, missing-directory refusal before file effects, and the
existing fresh-process seed recovery. It also checks the optional and default
Host builds. The checkpoint file tests use fixture JSON and fixture Host offers
and grants: they do not validate a recording catalog's committed chunk domain,
admit a live Body's disk capacity, or establish an overnight recorder service
(#6359 and #6360).

`PreparedRecordingCatalogCheckpoint::prepare_after_publications` prepares this
Source with recorder-domain catalog content only after every supplied publication
observer reports kernel commit for its exact generation. Missing, failed,
cancelled, mismatched or out-of-order publications refuse. The catalog's own
record and byte extent count toward the supplied finite ceilings; these ceilings
are constraints, not storage reservations. The lower-level `prepare` method
checks content and Source only and does not establish prior publication.

Catalog decoding validates the complete finite representation, reference profile
and uniqueness before replacing its previous view, preserving the encoded entry
order. It allocates no heap while reading; a malformed catalog preserves the
previous view. Publication tests use the production kernel and recording Back
with explicit CPU storage-completion fixtures. They establish the commit gate,
not actual file publication of those preceding chunks or their producer's
authenticated identity. Those facts still need the admitted recorder integration.
