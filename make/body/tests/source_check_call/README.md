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
