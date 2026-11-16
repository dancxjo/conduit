# Fresh selected Source execution handoff

This adds a local OS-file entrance for fresh execution requests against the SAME serving Root. It does not adopt Source, restart Root, replay retained policy one-shots, issue membership authority, or claim that transport negotiation proves Start.

## Runtime hook

In one synchronous turn (no await or cached witness):

1. Borrow the Owner's actual `selected_file_workset`; absence is a refusal, not a fallback to a Source path or reconstructed capture.
2. Call `source_revision::source_execution::verify_retained(state, &owner, original)`.
3. Consume that opaque non-Clone/non-Deserialize witness with `claim_and_execute(state, &mut owner, basis)` while the original field remains installed.
4. Retain any returned `RetainedExecution` beside that SAME Owner. Its outcomes are diagnostics; the value owns completed/pending/error carriers, including uncertain Return descriptors.

The execution entrance calls the Owner's `verify_selected_source_execution_basis` again. That hook borrows ONLY the actual installed original field and invokes `basis.verify_original`. Original Source/caller IMAGE guards, workset, current Body, Host/Boot/generation and immutable preparation fingerprint must all match. The policy bytes and full installed policy are rechecked before claim/open.

## Local request

`conduit host service execute-source REVIEWED.json --state-dir STATE` publishes a complete private request through a create-new staging file plus fsync/no-clobber hardlink. It takes no network control token. The schema is `conduit.local-source-execution@1`; fields are:

- exact policy SHA-256, Root Host/Boot/offer generation, installed release/configuration;
- exact current Body biography, Source snapshot and entry list;
- local expiry (at most five minutes from enqueue/consume);
- fresh bounded `plays` and `returns`, using the existing selected USB policy shapes.

There are no admissions or continuation outputs in this request. Old policy one-shots are never inherited. The Return basis must already exist when this fresh request is consumed; the new path does not poll for it.

The fixed inbox has ONE claim slot. Claim becomes durable before parse/effects. Invalid/expired requests and crashes never replay automatically; a claimed slot blocks subsequent requests and has no automatic replacement/retirement API. That conservatism is intentional pending a qualified retirement operation.

## Authority and ownership

The 32-byte original preparation fingerprint includes Root Host/configuration/release, File, snapshot, entries, source ceiling, resident preparation, capture/caller ceilings, local Fore intents and text-generation selection. It excludes only pre-adoption biography and retired one-shot actions. Both adoption and restart retain it with the actual guards. A replaced policy file with matching snapshot cannot silently replace original preparation identity.

Canonical duplicate-reader preflight precedes Return effects. Return and its later Play may share the SAME original descriptor; aliasing duplicate readers is refused. Failed returned-carrier selection leaves its descriptor in the original owner. A Return socket is opened outside all subsequent fallible observation/authentication/commit/receipt/delivery work, so uncertain failure returns owned descriptor custody rather than reopening.

## Explicit remaining gates

- The existing adoption wrapper still maps a retained Return failure to its legacy string error; the new execution entrance preserves the owned failure. Wider adoption error-lifetime repair is outside this patch.
- N must wire the synchronous hook and retain the opaque result; no live Root/device change is part of this patch.
- No active carriage borrow is exposed. A consumer/history pump requires separately qualified original Fore binding and actual Start evidence. Handshake is not Ready/Start/application delivery or durable recording.
- Physical USB/provider custody is unchanged. A remote TTY does not bypass local sysfs requirements.
- Pure fingerprint sensitivity tests cover funding/Root/path changes; genuine existing IMAGE restoration/Owner tests are rerun. A full new-entry test replacing a policy while the actual retained guards remain installed is still an N integration gate.
