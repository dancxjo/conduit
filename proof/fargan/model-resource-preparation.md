# Model resource adoption preparation admission

`AdmittedModelResource::storage_reservation` and `adopt_with_storage_limits`
check requested preparation and full retained payload ceilings before original
signature/descriptor digest encoding or authority cloning. Signature validation
now performs equivalent borrowed duplicate scans. One shared byte writer/counting
sink computes the exact unchanged v1 signature encoding length without allocation;
signature and descriptor encoders reserve that exact size before writing. No
model digest, Source declaration, compatibility, content or authority guard changed.

Retained inventory includes complete artifact strings/reference metadata, full
signature strings and all spare bounded sequence slots, complete model backing
payload, and newly cloned access identities. Original descriptor/signature/model
storage is moved unchanged. Shared storage dedup is available; outer owner roots,
Arc headers, allocator bookkeeping and stack remain separate charges. Input
construction and Source family Native admission remain upstream; this helper is
not a Source admission receipt or compute-readiness grant.

Three tracked fixture tests pass through a fresh complete prepared ModelSignature
family and original 3,272,868-byte FARGAN f32 model. The full 18,994-byte original
signature reencodes exactly. Archived signature c333ce46… and model descriptor
31b4af1d… remain exact. Signature encoding requests692 bytes; adoption requests
1,030 bytes, peaks692 bytes and retains45 new bytes. The complete retained payload
is3,276,591 bytes. Both exact-one-under ceilings refuse with zero allocation;
content, authority and signature corruption preserve original refusals.

The library and tracked test Clippy `-D warnings` pass. Fixture tests explicitly
require the original external `CONDUIT_FARGAN_MODEL_FIXTURE`; the small original
Native signature metadata fixture is checked in. The coherent compiler-closed
artifact/source graph is preserved by content hashes in the goal project's
outputs/fargan-model-resource-preparation. No whole runtime, trained-session,
model quality, target or #5212/#5215/#5218 acceptance is claimed here.
