# Untrusted boundary hardening version 1

Status: normative
Conformance profile: `conduit.c5`
Parent issue: #29

## Boundary and non-goals

Conduit accepts authored source, semantic descriptors, exact plans, packages,
implementation artifacts, host observations, realm/passport observations,
grants, carrier frames, execution evidence, and Patchbay protocol messages from
different trust domains. Validation of one identity does not validate another:
a correct source hash is not a plan, a signed artifact is not authorized or
confined, a passport is not a grant, carrier authentication is not Conduit
authority, and evidence is not a semantic value.

This contract protects Conduit parsers, validators, queues, and failure
records. It does not claim that native code is safe, that a signature makes
code benign, that TLS grants an effect, or that software alone guarantees
physical safety.

## Assets, attackers, and assumptions

The protected assets are semantic and plan integrity; grants, secret handles,
and protected values; bounded CPU, memory, storage, timers, queues, retries,
and evidence; run attribution and terminal truth; host and realm identity; and
the safety authority retained by domain hosts.

An attacker may author or replace any untrusted input, control a mirror or
carrier, replay a previously authentic observation, operate an authorized but
malicious realm member, possess a valid signing key, race a mutable artifact
path, exhaust observers, or provide foreign code whose manifest lies about
memory, cancellation, task creation, isolation, or ambient authority.
Cryptographic implementations, operating-system sandboxes, protected key
stores, TLS termination, and physical interlocks are host services whose exact
identity and observations must be named by a plan. Their existence is never
inferred.

## Mandatory allocation ceilings

Limits are checked before retaining the corresponding variable-sized
structure. All aggregate size arithmetic is checked.

| Boundary | Version 1 ceiling |
| --- | --- |
| ordinary `.panel` source | 4 MiB, 262,144 tokens, 64 nested value constructors |
| exact compile input/source closure | the explicit limits in specification 036, never above 16 MiB input, 4 MiB per source, 8 MiB closure, or 256 modules |
| hosted Serde JSON | pinned decoder recursion ceiling 128, plus the owning byte/item limits |
| canonical descriptor nesting | 64 |
| safe inspection | 8 MiB input, 1 MiB record, 4,096 records, depth 64, 16,384 collection items |
| package envelope | 256 MiB package, 1 MiB manifest, 4,096 objects, 128 MiB per object, 256 MiB aggregate extraction |
| execution evidence NDJSON | 8 MiB stream, 1 MiB record, 4,096 records, 64 KiB inline payload, core derivation maximum, 4 KiB per accepted string |
| distributed carrier | exact plan-selected payload, frame, send/receive/retry/reorder/dedup, attempt, timer, and evidence budgets |
| embedded HIL frame | exact fixed codec size and protocol version |

Smaller caller or plan limits remain authoritative. Archives, compression, and
path-bearing package entries are not supported in package version 1, so an
archive or decompression bomb is rejected rather than expanded. Future codecs
must add compressed and expanded byte limits before acceptance.

## Source, descriptor, report, and plan validation

Source byte and token ceilings precede AST allocation. Recursive literal
constructors fail with `CND-SEC-002` before descending beyond 64. Module
resolution remains an explicit, digest-pinned, bounded closure and cannot
escape into ambient filesystem or network discovery.

Hosted JSON is byte-bounded before Serde allocation. Unknown fields fail.
Core descriptor and exact-plan validation remains allocator-free with
caller-provided scratch. Checked identities do not excuse stale capability
reports, passport status, grants, resolver policy, plan epoch, or host
resources. Forged, rolled-back, cross-realm, cross-audience, expired, revoked,
or over-depth membership and delegation observations fail under their owning
#10, #26, #27, and #88 reason vocabularies.

Diagnostics may report stable codes, byte counts, field names, digests, and
escaped locations. They do not reflect arbitrary hostile text, secret
references, protected digests, values, headers, keys, credentials, or payload
bytes.

## Artifact trust and load handoff

Digest, byte size, target, ABI, selected trust policy, and external signature
observations are checked before a loader sees bytes. The hosted
`VerifiedArtifactBytes` gate owns the verified allocation and consumes it into
one loader callback. A mutable path or caller buffer therefore cannot be
substituted between that check and handoff.

A rejection emits bounded terminal gate evidence containing the manifest
identity, candidate digest, observed size, stable reason code, and zero
reflected payload bytes. A successful signature answers only who vouched for
those bytes under a selected policy. It does not prove safety, manifest
truthfulness, semantic compatibility, current authority, or isolation.
Revoked/rotated/wrong-scope signers, signer confusion, project substitution,
false manifests, online-key compromise, and verification/load races remain
separate failures.

## Runtime, carrier, and implementation isolation

Every runtime boundary rechecks current authority when its plan requires it.
Oversized, wrong-epoch, wrong-binding, unauthorized, or capacity-exhausting
frames mutate no queue and produce bounded `FrameRejected` transport evidence
when the plan's evidence reservation permits it. Evidence exhaustion itself
fails before a host effect or queue mutation. Replays, duplicates, reorder,
retries, reconnects, cancellation, and terminal acknowledgements remain owned
by specification 037 rather than a security-specific second session machine.

| Implementation kind | Honest version 1 isolation statement |
| --- | --- |
| native in-process / C FFI | process authority; memory corruption, blocking, thread creation, ambient I/O, and undeclared retention are not confined by Conduit |
| process | isolation is only the named process executor, OS policy, handles, namespaces, quotas, and current grants |
| WASM | isolation is only the named engine, component/import policy, memory/fuel/epoch limits, and host calls |
| remote | isolation is the remote host plus exact authenticated carrier, realm/passport, grant, and fresh host observations |
| firmware | isolation is the exact firmware, hardware protection, static pools, peripherals, transport, and physical interlocks |

Foreign progress, wake interests, state exports, and operation replies remain
bounded by specification 022. A future live transition must reject oversized
or secret-bearing handoff, stale epochs, endpoint races, unauthorized
requests, overlap exhaustion, rollback confusion, and downgrade attacks. It
cannot call HTTPS-to-HTTP, authenticated-to-unauthenticated,
secret-to-public, bounded-to-unbounded, or required-delivery weakening
"graceful degradation."

## Network and web entry gates

Issues #41 and #42 are not implemented by this contract. Until their exact
backends exist, Zenoh, HTTP, HTTPS, TLS termination, forwarding trust, and
upgrade profiles fail unsupported during resolution. Their entry gates must
add retained abuse corpora for stale session epochs, oversized/replayed
frames, duplicate floods, scouting surprises, ACL mismatch, TLS/mTLS
downgrade, invalid certificates, opaque-secret misuse, hostile SNI/Host/header
values, slowloris timing, oversized/chunked bodies, upgrade abuse, and
untrusted forwarding headers. A reverse proxy is trusted only when its
boundary and identity are pinned in the exact plan.

## Realm/passport abuse boundary

Passport validation and status do not confer authority or honesty. Required
negative coverage includes forged or rolled-back passports, cloned software
keys, replayed enrollment, issuer/realm/audience confusion, stale revocation,
compromised roots or members, unauthorized role reassignment, split-view root
rotation, emergency-root downgrade, bounded trust chains, Sybil
re-enrollment, federation confusion, transitive-trust accidents, bridge
authorship laundering, and identifier/fingerprint privacy leakage. Hosts
without protected key storage cannot claim non-exportability, clone
resistance, or strong attestation.

## Automation and retained reproducers

Every CI run executes the reviewed security vector corpus as ordinary tests.
The scheduled and manually dispatchable Security workflow runs libFuzzer
targets for panel parsing, exact-plan JSON and validation, package envelopes,
evidence NDJSON, and the embedded transport codec. Failure artifacts are
retained for 30 days and become permanent regression seeds when reviewed.
The same workflow performs advisory, license, dependency, and source-policy
review. A clean advisory scan is evidence about a database observation, not a
proof of safety.

## Stable reasons

- `CND-SEC-001`: source or untrusted document allocation ceiling exceeded
- `CND-SEC-002`: recursive source value ceiling exceeded
- `CND-SEC-003`: hosted size representation overflow

Owning schemas retain their existing `CND-SRC-*`, `CND-CMP-*`, `CND-PKG-*`,
`CND-MAN-*`, `CND-ART-*`, `CND-AUT-*`, `CND-DST-*`, `CND-EVD-*`,
`CND-RLM-*`, and `CND-PBY-*` reasons. Security hardening does not create
parallel failure vocabularies for those contracts.

## Requirements

| ID | Requirement |
| --- | --- |
| SEC-001 | Keep source, descriptor, plan, artifact, host, realm, authority, transport, evidence, and presentation identities distinct |
| SEC-002 | Reject untrusted input beyond explicit byte, depth, item, retry, queue, and evidence limits before unbounded retention |
| SEC-003 | Use checked arithmetic for aggregate sizes and budgets |
| SEC-004 | Structurally redact secrets and avoid reflecting hostile bytes in diagnostics, logs, evidence, crash context, or UI |
| SEC-005 | Bind verified artifact ownership through the immediate loader handoff and retain trust policy separately from signatures |
| SEC-006 | Recheck current authority at effect boundaries; signatures, passports, carrier authentication, and locks do not grant effects |
| SEC-007 | Preserve bounded trustworthy rejection and terminal evidence before side effects or queue mutation |
| SEC-008 | State actual native, process, WASM, remote, and firmware isolation without blanket sandbox claims |
| SEC-009 | Fuzz parsing, exact plans, packages, evidence, and transport codecs with retained reproducers |
| SEC-010 | Review dependencies, advisories, licenses, and sources without treating a clean scan as proof |
| SEC-011 | Fail unsupported network, web, transition, and degradation profiles before start until their owning contracts land |
| SEC-012 | Keep physical-safety enforcement in domain hosts and physical interlocks rather than delegating it solely to Conduit |
