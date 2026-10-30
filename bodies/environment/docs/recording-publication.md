# Recorder snapshot publication

`RecordingSnapshotBack` validates one prepared, fixed-cardinality recording chunk
and delegates its publication to the existing kernel `HostCallBack`. Root still
selects and admits the actual snapshot implementation, resource, placement,
authority, queues and complete work/storage closure before starting a Play.
Preparation and matching identity bytes do not create that authority.

`RecordingContext` selects a `RecordingTimestampBasis` independently of incoming
content. Acquisition retains the existing `producer-boot-ms` representation;
producer-observation time uses `producer-observation-boot-ms`. These are event
meanings in the original producer Boot clock, not UTC or recorder receipt time.
Readers refuse a different basis without replacing visible history. The shared
history Face and its browser/native graphs preserve the distinction through
`sampled-at-boot-ms` or `observed-at-boot-ms`, and refuse conflicting properties.

With `face-projection`, `append_observed` accepts the shared canonical
`ObservedRecord` only into an observation-time chunk or snapshot. It preserves
the exact 21-byte record, including failure, sequence and observation time,
without allocating. It supplies no signature verification, original producer
identity, storage grant or acknowledgement; the live recorder must establish
those independently before this conversion.

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
composition, bounded allocation-free reference expressions, committed catalog
validation, actual chunk/catalog file publication feeding installed durable
State, refusal preserving the previous seed byte for byte, and missing-directory
refusal before file effects. A separately executed child receives storage paths
only, discovers the catalog reference from State and recovers 15 chunks / 960
observations through one ordered production kernel. Its exact recovered-reference
receipt is required before the parent accepts success. Optional/default Host
builds, explicit timestamp bases and the shared history Masks are also checked.
Host offers, grants and producer identities remain CPU fixtures: this does not
admit a live Body's disk capacity or establish an overnight recorder service
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
previous view. Publication tests use the production kernel and recording Back,
with both CPU completion fixtures and actual Linux file effects. The observer
currently names the selected reference and committed sequence range; binding
exact committed content remains #6465's separate obligation. Actual producer
authentication and live resource/authority admission still need the recorder
integration.

## Live producer provenance

The signed observation journal binds Body, original producer Boot and Play
digests. Those digests do not recover the original Host/Plan/Play identities.
Root must verify a candidate canonical ActivePlay tuple against the independently
authenticated epoch commitment, with the original Body, Host, Boot and producer
key pinned independently. A candidate delivery cannot replace those pins. Exact
canonical correspondence establishes the original identity tuple, not admission
or execution authority; it does not require reauthorizing the old Play or
recovering its complete Plan. Retain the original signed evidence and matched
tuple for restart. A current-producer recovery receipt establishes current Boot
identity; it does not supply the historical tuple or replace that check.

Journal `observed_at_boot_ms` is producer-observation time, as its descriptor
states. It is neither acquisition time nor UTC. A journal adapter selects
`RecordingTimestampBasis::ProducerObservation` and uses `append_observed` after
authenticating the original record and producer context. Recorder receipt time
and receiver wall time cannot supply missing provenance or replace its clock.
