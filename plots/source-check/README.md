# Check an authorized immutable Source

`main.conduit` forwards an exact S1 request to `language/source-check` and exposes
its descriptive report. The finite authoring context must supply the actual
checked Types from `semantics/language/source_check_info.conduit`. Type names,
reports and source hashes confer no access, installation or execution authority.

The local std reference prepares canonical Rust analysis for an explicitly
bounded set of immutable requests. Its initialized native provider retains
only admitted request/report packets. The ordinary selected native Source slot
and Host Call enforce current Host/Boot/provider/resource/grant binding; Play
matches and copies a prepared packet without invoking the compiler or allocating
inside the provider. Another request requires new authorized preparation.

This is local implementation for #6447, not a completed or deployed feature.
Compiler/catalogue/workspace admission remains separate from retained packet
bounds. Generic provider refusal is not yet the full typed S1 refusal protocol;
cancellation, resource and provider-loss acceptance remain incomplete. There is
no automatic offer on browser, firmware or ordinary unconfigured native Hosts.
The checked candidate is never played by this Plot. #6449 owns authorized Body
workset adoption, independently of checking.
