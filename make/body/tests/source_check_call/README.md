# Check an authorized immutable Source

`check.conduit` forwards an exact S1 request to `language/source-check` and exposes
a typed reply containing its descriptive report, an S1 refusal, or an explicit
`not_prepared` result. The finite authoring context must supply the actual
checked Types from `semantics/language/source_check_info.conduit`. Type names,
reports and source hashes confer no access, installation or execution authority.

The local std reference prepares canonical Rust analysis for an explicitly
bounded set of immutable requests. Its initialized native provider retains
one shared request schema, one shared reply schema, and bounded canonical
request/result nodes. The ordinary selected native Source slot
and Host Call enforce current Host/Boot/provider/resource/grant binding; Play
matches and copies a prepared packet without invoking the compiler or allocating
inside the provider. A different request within the same Source/context returns `not_prepared` and
requires fresh authorized preparation; stale Source and unknown context return
distinct typed refusals.

This package-owned conformance Source exercises local implementation for #6447;
it is not a reviewed public Plot or a completed/deployed feature.
Compiler/catalogue/workspace admission remains separate from retained packet
bounds. The immediate provider returns typed stale/context/malformed refusals;
pre-call cancellation suppresses the call and a later authorized Play succeeds.
Old Plans refuse actual missing/replaced providers, changed Boot and replaced
backing pools. Preparation can also observe the existing RunControl between compiler/schema
stages and return no provider after observing a stop. It does not interrupt an
individual compiler call, retire the caller-owned compiler cache, or mark a Play
quiescent. Hard workspace/exhaustion acceptance remains incomplete. There is
no automatic offer on browser, firmware or ordinary unconfigured native Hosts.
The checked candidate is never played by this Plot. #6449 owns authorized Body
workset adoption, independently of checking.

The Linux process reference additionally constructs the actual product catalogue
inside a memory/CPU-limited child. Its parent uses existing bounded process
polling, Stop/deadline and reaping. A completed process returns descriptive
bytes, not a compiler owner or Source grant.

The selected-IMAGE integration is exercised in
`products/conduit/tests/source_check_process/image.rs`: initialization reserves
the declared provider preparation/backing before invoking the actual compiler;
the selected Body execution entrance then reserves a managed-memory receipt
before Start. Valid and invalid candidate Sources return the same typed result
as the direct canonical reference, through real Host Call events. Execution and
cancellation allocate nothing after Start. The unaccounted execution entrance
continues to refuse; it must not be enabled by removing its guard.

`targets/std/src/body_execution/image/tests/native_source.rs` separately checks
one-byte-short refusal before Start, exact-budget acceptance, retained result
custody after the run report is dropped, and refusal of an independently
uncharged reference provider. This uses an immediate fixture provider, not the
compiler process, and does not prove full Root preparation high-water.

These are integration fixtures, not an installed authoring feature. The product's
`durable_host/runtime_preparation.rs` still constructs selected timer/native-Mask
Hosts without a Source-check provider. Ordinary owner selection of authorized
Source, compiler initialization, typed request/result routing, and the complete
Root preparation bound remain acceptance gates for #6447. Keep those gaps
distinct from the managed Body execution receipt that already exists.

The ordinary product Source catalogue now includes the checked request/reply
Types and `language/source-check` declaration without requiring a prepared
checker instance. `install_authoring_declaration` shares registration with the
prepared checker and installs no provider, resource or grant. The product test
in `plot_source/source_check_catalog.rs` checks and expands this Source through
the ordinary loader, then verifies that placement on an unconfigured Host still
refuses. This makes the operation authorable; it does not complete installation.

The product's Linux `source_check_worker::initialize_image` now owns the actual
initialization sequence previously assembled in the process fixture. Root
supplies its selected File, explicit Source scope, current native definition and
independently held grant. Preparation/retention and request/reply envelopes come
from the checked Host configuration. Source capture and compilation happen
inside the existing funded initializer; the configuration, catalogue, schema
and stop control are borrowed and cannot escape into the returned Host.
Envelope drift and a preexisting stop refuse before Source capture. This is an
explicit unsafe Root correspondence boundary, not a new authority constructor
or an installed owner action. The process IMAGE integration test uses this
product routine and still compares the resulting Body Play's typed bytes with
the direct canonical reference.

Successful product initialization returns the Host together with the exact typed
request derived from that same capture. A caller need not reread Source or
reconstruct its snapshot to query the prepared provider. Extraction moves the
request and releases the captured Source bytes without allocating. The returned
request is descriptive metadata whose retention/copying/routing Root must
independently admit; it grants no Source access or candidate adoption. Failed
initialization returns neither a Host nor partial selection metadata.
