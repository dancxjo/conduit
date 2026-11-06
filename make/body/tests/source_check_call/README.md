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
