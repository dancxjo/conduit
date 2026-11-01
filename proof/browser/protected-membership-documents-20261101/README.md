# Protected membership bounded documents

The existing CBD1 codec now has a reusable Rust protected-carrier document
adapter and a matching browser session adapter. Complete logical documents are
bounded by the existing 2 MiB cap; protected traffic limits are unchanged. Both
sides verify SHA-256 before returning completed bytes, serialize chunks within
one logical send, and use one absolute deadline starting before the first
receive. Uncertain/failed transfers retire; no retry or ambient fallback exists.

Large storage is independently selected, not inferred from frame quotas:

- Rust: `crate::host_rendezvous::{ProtectedDocumentLine, DocumentWorkspace}`;
  `new(retained_carrier, timeout_millis, Option<DocumentWorkspace>)`, then
  mutable `send`, `receive`, `close`. Workspace fields are
  `maximum_document_bytes` and `capacity_bytes`.
- JS: SDK `participate` / `joinBrowserBody` accept
  `sessionDocumentWorkspace: { maximumDocumentBytes, capacityBytes }`;
  `createBrowserMembershipSession` takes that descriptor as `documentWorkspace`.
  Default absence preserves only the original small-frame carrier path.

These sizing values confer no authority or admission. Root must derive them
from actual independently admitted workspace. JS charges queued immutable
snapshots, scratch, assembly, digest backing and conservative WebCrypto input
copy to one selected budget. A selected maximum document extent does not waive
insufficient simultaneous storage. Completed reassembly remains charged until
the membership consumer finishes. Rust borrows sender input, bounds receiver
assembly plus frame scratch, and moves completed bytes without a full copy.

The static registry and SDK package include the complete reachable membership
module imports. The candidate parser has one canonical host asset and a
workspace API reexport. Source graph tests cover source/staged import identity.

## Exact local qualification

47 focused JS tests pass at the JS source in receipt: codec boundaries and
smaller frames; synthetic 701,632-byte/11-frame roundtrip; exact CBD1 fields;
changed payload/digest, reordered/duplicate/mixed/truncated input; absent or
insufficient workspace before allocation; captured sizing and shared queued
snapshot budget; initial/multi-chunk deadlines; held final chunk cancellation;
existing membership/Part/provision behavior and exact staged/SDK closure.

8 Rust tests pass at `bedf2b5b73` in the existing protected-memory fixture:
701,632 synthetic bytes in both directions; unchanged protected frame quotas;
hash/mixed rejection; initial/multi-chunk deadlines; absent/insufficient selected
workspace and original raw small-message compatibility. The later JS-only
cancellation guard does not change Rust implementation/test sources.

Command: `cargo +stable test --manifest-path products/conduit/Cargo.toml
--features installed-environmental-sources --bin conduit
protected_membership_document_ -- --nocapture --test-threads=1`, with the exact
environment in receipt. Native cache was explicitly released after completion.

JS command: `node --test` for host assets `browser-bounded-document.test.mjs`,
`browser-membership-session.test.mjs`, `browser-membership-closure.test.mjs`,
`browser-spore-provision.test.mjs`, `browser-spore-membership.test.mjs`, plus
`sdk/package-browser-bundle.test.mjs`. Compressed logs are retained alongside.

## Explicit integration gate

The audited base has no actual protected BrowserAdmissionIngress owner service
bridge. The reusable Rust adapter is intentionally not wired into unrelated
hello/invite host rendezvous production traffic. No existing live caller
silently selects the new workspace. Root owns binding the real retained
protected carrier and actual endpoint/line evidence to the existing serialized
owner admission service and deriving its workspace from real admission.
Existing loopback Socket evidence was not repurposed.

The 701,632-byte tests are synthetic sizing/custody fixtures, not an actual
Rust-generated 140-offer browser advertisement or invitation proof through a
live owner service. Actual WASM, owner service membership, cryptographic Body
receipt/admission, live relay, physical display and hosted acceptance remain
separate gates. No original physical Source failure cause is claimed.
