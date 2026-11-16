# Owner authority across restart — bounded audit, 2026-11-15

**Verdict: the current USB refusal is correct. No existing audited API can lawfully advance this MCU's independently installed owner pin to the fresh standard Root peer.** Durable Body/member continuity already exists, but it is not a chain authorizing a different owner Boot/key. Existing continuation artifacts authorize a bounded recipe; they explicitly do not establish owner-confirmed membership or replace the owner pin.

Read-only at integration HEAD `816230877a05e1e9e3c21e8e2bb7f55d460ce9f7`, including current working source. No builds, code changes, control IPC, device I/O, key inspection or live-owner operations. Parent supplied the exact live mismatch: old Blue basis pins `boot/local-source-policy/c5f5` and old key; standard Root746 currently advertises Boot6844 and a new key. This audit independently checked the source law, not live observations.

## Where the chain stops

1. `products/conduit/src/durable_host/usb_member_return.rs` validates the supplied return basis and compares `carrier_receiver(state)` to **the entire** `basis.owner` before opening the carrier. Refusal therefore cannot be repaired by a USB retry, member stale-credential recovery policy or delivery adjustment.
2. `body_owner/member_carrier.rs::prepare_current_member_carrier_receiver` creates a random `SpawnInvitationSecret` in per-Owner-instance `MemberCarrierCustody`. `member_carrier_peer` combines that key with the current actual Host/Boot/offer generation. This key is an ephemeral current receiver identity, not a persisted installation authority.
3. `attest_current_member_receipt` independently checks current member/observed offer, exact committed grants, continuity witness and equality of expected owner with that current peer, then signs with the current carrier key. It correctly cannot sign a fresh receipt as the old peer.
4. `architecture/body/src/admission_receipt_attestation.rs` binds exact receipt bytes, owner Host/Boot/generation/key and member key. `targets/mcu/host-runtime/src/member_return.rs::RetainedReturn::exchange_inner` verifies metadata against **immutable `bootstrap.owner`** before decoding/accepting the assembled receipt. Editing the public CLI basis would not change this installed trust anchor.

`MemberBootstrap` stores a complete `FederationPeer`, not merely an owner Host name or stable signing-authority identifier. Consequently even preserving the same carrier key would not authorize a genuinely fresh owner Boot under current exact-peer checks. Reusing an old Boot would falsify current execution identity.

## Reusable existing mechanisms, and what they do not authorize

| Mechanism | Existing continuity | Boundary still missing |
| --- | --- | --- |
| `AdmissionManager` / `admission/continuity.rs` | Retains a Part's member verifying key; validates fresh bounded PartReturn proofs for a real new member Boot. | Does not attest a new owner peer to a receiver pinned to the old owner. |
| `body_owner/state.rs` and `state_admission_custody.rs` | Durable recoverable owner transaction retains biography, installation, admission manager and bounded archived evidence. | Inspected transaction has no durable private owner-root signer or signed owner-peer succession record; a durable biography alone is not such authority. |
| `durable_host_membership/continuity_key.rs` | Privately retains the original **member's** invitation key, scoped to Body/Part/Host/invitation; signs authenticated fresh Return challenges. | Module explicitly says this key does not authenticate an owner receipt or authorize Return/Line. It cannot become the Body owner's signer. |
| Owner-local recovered-member return policy | Selects the latest independently durable member credential while treating stale wire credential as identity hint; current member-key proof remains mandatory. | Addresses member-side lost ACK/old credential, not owner key/Boot pin change. |
| `continuation_intent.rs`, `continuation_artifact.rs`, MCU continuation acceptance | Separately installed issuer Host/key and explicit bounded recipe authorization; locally accepted generations and actual new-Boot facts are checked. | Documentation explicitly excludes membership and Start authority, and says DTOs do not certify durable media, current owner-confirmed membership or rollback protection. Do not reinterpret a recipe signature as owner succession. |
| `body_owner/member_carrier/continuation.rs` | Signs an exact original checked Source/recipe/Body scope with the current private carrier key. | Uses the same per-Owner carrier key; it neither persists an installation authority nor certifies a successor receiver. |
| Crèche retained factory / ordinary Seed composition | Retains checked IMAGE custody and packages exact Body/Host/Seed material under independently supplied authority. | In the inspected branch, factory persistence/IMAGE custody is not a durable cryptographic Root signer or a verifier for advancing an already installed owner anchor. A new Seed's embedded peer does not amend another installed Seed. |
| Current-member federation challenge | Binds member proof and exact trusted receiver/session/Line/time/frame facts. | Discovery/current advertisement is evidence, never authority to replace trusted receiver facts. |

The bounded search covered current Root/member/Seed/Crèche and continuation entrances. This is an absence finding for these audited APIs, not a claim that no future canonical authority abstraction could supply the missing operation. In particular, another branch's proposed Womb/issuer implementation must be reviewed as a distinct authority owner before relying on it.

## Smallest correct contract to add or connect

Keep the current `FederationPeer` session identity fresh. Introduce/connect an **independently installed durable authority that may authorize that fresh peer**, rather than making the peer/session key itself durable by accident. A receiver needs a checked, bounded authorization transition from an anchor it already trusts to the exact new Body-owner Host/Boot/generation/key, with explicit scope, freshness/issuance identity, revocation/retirement and interrupted durable acceptance semantics. Authorization for membership receipt issuance must be explicit; recipe permission or native execution grants are insufficient.

This is a missing authority/custody transition contract, not a recommendation to invent an ad hoc USB message. First reconcile it with the project's Root/Seed issuer owner and existing transcript/delegation APIs. If an existing durable issuer is intended to own this authority, its typed signer must issue only the specific checked transition, and MCU/Linux verification must consume that typed result. A raw signing endpoint, caller-supplied replacement pin or transport-discovered key is not an implementation.

Crucially, the currently installed old pin must already delegate this kind of succession, or an independently authorized local installation must replace the Seed/bootstrap. A newly created unrelated authority cannot retroactively authenticate itself to the old device. If the old private authority is lost and no installed delegation exists, an ordinary Return cannot bridge that trust gap. An explicit reviewed reseed under the human's authorized physical deployment is the available trust-establishment path; it must be described as a new installation, preserving archived evidence and required Body/Part continuity rather than claiming a successful old-anchor Return.

Do not rewrite `basis.owner`, patch flash pins behind Root, retain/reuse an old Boot, persist the ephemeral carrier key as a workaround, bypass metadata verification, or treat same hostname/Body label as cryptographic authority.

## Existing issue ownership

Read current issue bodies once via GitHub connector:

* [#6340](https://github.com/dancxjo/conduit/issues/6340) requires active native owner Inspect/quiescence to remain bound to the exact admitted reservation across **offer-generation changes**, while preserving Boot/key/sequence authentication. Its body explicitly separates membership retirement and native reservation authority. It does not authorize a fresh Boot/key or own a generic credential-persistence feature.
* [#6409](https://github.com/dancxjo/conduit/issues/6409) requires A→fault/retire→B non-resurrection, stale-key/Boot/identity refusal, interrupted replay/ACK and no preserved physical authority merely because semantic Body survives. It is a correctness gate for any owner succession path, not permission to revive old carrier state.
* Existing joining/Root owners #6461 and constrained receiver #6770 are the integration lane for this missing owner-return authority transition. Use those owners and the existing Root/Seed authority design; no separate MCU memory epic or bespoke transport lane is indicated.

## Concrete file seams and qualification gates

Authority/typed transcript owner: adjacent to `architecture/body/src/member_bootstrap.rs`, existing attestation/current-member/delegation modules, **only after** defining which independently durable Root/Seed issuer owns succession. Do not overload `ContinuationIntent` with membership meaning.

Linux durable signer/transaction owner: `products/conduit/src/body_owner/{controller.rs,state.rs,member_carrier.rs}` and an explicit authority helper if required. Keep admission commit-before-ACK; do not expose secret keys. `durable_host/usb_member_return.rs` may carry a verified authorization chain once Core defines it, but must not be its source of trust.

MCU receiving/persistence owner: `targets/mcu/host-runtime/src/{member_return.rs,root*.rs,member_bootstrap/seed custody}` as appropriate; preserve immutable installed trust or explicitly authenticated durable advancement. Every actual media-write/reopen/rollback property needs its owning target proof, not merely a serial exchange success.

Tests belong beside existing `architecture/body/src/admission_receipt_attestation.rs` / federation-current-member tests, `products/conduit/src/body_owner/tests/{member_carrier.rs,member_return.rs,state tests}`, and MCU member-return protocol tests. Require: same durable authority with genuinely fresh Root Boot/key; original installed anchor; exact Body/owner/member scope; wrong/self-signed successor refusal; changed Boot/generation/key tampering; stale/replayed/expired/cancelled transition; revocation and concurrent successor; interrupted commit/ACK/reopen; old carrier/receipt cannot become current; ordinary fresh member Return still required; local Play/reservation custody stays untouched on refusal.

Immediate outcome: retain the correctly refused pre-device-open evidence and continue independent preparation work. This audit does not qualify Return, authorize a pin change, or establish that the MCU has adopted the new Root.

## Addendum — does the owner Host already have a durable key? (bounded follow-up)

**Joined Host: yes, a scoped member continuity key exists. Born Body-owner Host: the inspected construction/reopen path does not provision one. Neither finding supplies an existing receipt-issuer authority API.** No live Root/state-directory I/O was performed, so this is a source-path finding, not a claim that a particular physical file is absent on Root746.

The follow-up stayed within20 source files and performed no builds or changes outside this addendum. Relevant additional sources: `products/conduit/src/durable_host_membership{.rs,/continuity_key.rs,/continuity_key/tests.rs,/returned.rs}`, `durable_host_control/body_birth.rs`, `body_owner/{controller.rs,controller/birth.rs}`, `durable_host{.rs,/installation_read.rs,/runtime_marker.rs}`, and existing preparation-delegation/canon references.

### Proven durable member-key custody

`durable_host_membership/continuity_key.rs::RetainedKey` stores schema, Body, Part, Host, invitation identity and the original invitation secret at **`body/member-continuity-key.json`**. This is separate from `MemberCarrierCustody.key`.

`complete_body_join_document` requires explicit membership authorization, validates the exact pending request and accepted receipt against installed Host identity, then calls `continuity_key::retain` before publishing joined membership. `retain_rendezvous_membership` has the corresponding pending-join path. `retain` independently rechecks the pending invitation/request, exact current admitted advertisement and signature produced from that same invitation secret. Existing different identity/scope refuses overwrite. Publication uses a private staging inode, file sync, atomic no-clobber hard-link publication and directory sync; crash retry validates the existing exact record. Tests include key-publication-before-membership interruption and restart completion. Returning membership calls `validate_member_scope` and commits the new credential separately; it does not replace the key on each Boot.

Reading requires actual directories, a bounded nonempty regular record (maximum4096 bytes), Unix0600 permissions and matching opened inode/device metadata. Temporary serialized buffers and record secret fields are cleared on their defined cleanup paths. These are the existing finite filesystem custody rules, not a new cryptographic revocation guarantee.

There is deliberately **no raw key getter or generic signing endpoint**. `read` and `RetainedKey` are private to the module. Two typed gates reconstruct the signer internally:

* `sign_return_challenge`: exact retained Body/Part/Host scope, exact actual new member Boot/offer matching the challenge, nonzero nonce and bounded current challenge lifetime; signs only the canonical PartReturn transcript. It proves the member's key possession, not that the member may issue receipts to other Hosts.
* `sign_member_carrier_challenge`: independently loads installed current credential, checks exact member key/actual current Boot/offer and trusted receiver/session/Line/security/frame/time facts, then signs only the existing federation transcript. It does not grant admission, presence, Line initialization or effects.

This key originated as the private invitation's member-admission secret. Its existing purpose and trust distribution must not be silently upgraded to an exclusive Root authority key. Possession/availability and permission to attest membership receipts are separate questions.

### Born owner path is different

`durable_host_control/body_birth.rs` calls `Owner::prepare_born`, journals `PreparedBirth::retain`, then transfers the actual installed Host into `Owner::from_prepared_born`. In `body_owner/controller/birth.rs`, the initial Part and presence are established through the locally authorized Birth and exact Host/Boot Sign context. `PreparedBirth` retains the lifecycle session and resident Plot; it generates/persists no member signing secret and makes no pending invitation join.

`durable_host.rs::Installation` persists Host identity and release/body bindings, not a cryptographic Host signing key. Reopening the owner resumes Body membership against the actual current advertised Host/Boot and restores the admission manager. The separate `member_carrier` signer is still created afresh as previously audited. Thus a born-owner source path does not automatically acquire the joined-member continuity file simply because it is also a Host/Part in the Body.

The current Root's actual ownership classification/state was not reread under this read-only/no-Root-I/O task. If it has separately completed a joined-Host installation, the durable member key could physically exist, but its availability would still need the exact existing scope checks and explicit receipt-issuer authorization. No such receipt-signing accessor or authorization role was found in this bounded source path.

### Evaluation of a new durable Host-authority anchor

**Yes, a new Seed can explicitly anchor a durable `(Body, authorized Host, verifying key)` identity while receipts name the actual current owner Boot and the exact committed returning-device Boot/grants, with the transport peer remaining fresh.** This is a coherent proposed **new typed authority contract**, not behavior already furnished by the current `FederationPeer` pin or existing member-key signer. The durable signer must be privately and recoverably owned, and authorized for that receipt-issuance purpose. A joined member's existing key could be considered only through an explicit role/purpose decision and typed signing gate; a born owner currently needs a lawful durable signer-custody path or separately installed authority. Do not expose the secret to caller code or rename the ephemeral transport key into this role.

The required acceptance obligations are:

* Independently installed anchor plus explicit Body-owner/receipt-issuer role. A matching Host name, Body label, public key, ordinary membership proof or discovery observation is insufficient.
* Domain-separated typed receipt/peer-authorization statements. Bind exact Body/issuer identity and current owner Boot/offer, exact member/Part/key/current device Boot/offer, original proof/receipt bytes and committed grant projection. Neither a transport ACK nor a member's own proof becomes durable admission.
* Actual current Boot comes from the issuer's owning Root, not an incoming caller; a durable key does not preserve old runtime leases, Plans, reservations, session sequences or effects. Transport authentication and current owner proof remain distinct from installed durable trust.
* Explicit key/role replacement, withdrawal/revocation and finite issuance identity. Persist accepted advancement/revocation before acknowledgment; reject stale chains and reused issuance identities. Account honestly for unknown remote revocation while disconnected rather than claiming omniscience.
* Atomic interruption/reopen semantics for signer publication, accepted authority state and receipt commit; no silent fallback to a newly generated key when a durable record is missing/corrupt. Preserve least-authority filesystem/IMAGE ownership and finite backing.
* A→fault/restart→B tests must retain the same durable authority while changing the actual owner Boot and transport key, prove old-peer/receipt refusal, and preserve member admission commit-before-ACK and reservation cleanup. Compromised-key recovery is an explicit installation/rotation policy, not key-file deletion.

Old Seeds still pin the entire old peer and do not opt into this proposed contract. The user's authorized operator reseed can establish the new anchor; it must not be presented as cryptographic succession from a lost old key. No issue, wire format, signer implementation or deployment was created by this evaluation.
