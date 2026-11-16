# Crib and Womb: implementation gap

Current terminology, directed by the user: **Crib makes Bodies; Womb makes
member Seeds.** Existing `creche` identifiers below locate the inspected
implementation and retained evidence; they do not name a second lifecycle.
The full lifecycle acceptance belongs to [#6911](https://github.com/dancxjo/conduit/issues/6911).

Source inspection: `dev` at `4400d3dffc`, 15 November 2026. This is a design
and source audit with the narrow deterministic qualification below, not a
browser execution, deployment, or release receipt.

The requested boundary is: Crèche establishes an unbodied Host's belonging
through explicit Birth or authenticated Join; Womb propagates the existing
Body through ordinary checked Plot execution. Preparing media never establishes
Boot or membership. Independent reproduction requires a separate explicit Birth.

## Existing behavior and proof limits

| Surface | Confirmed implementation | Remaining requirement |
| --- | --- | --- |
| [Zero-Body front door](../../plots/patchbay/workbench/model/src/zero_body_front_door.rs) | Explicit opening, Birth and Join, with revision and observation freshness checks. [Tests](../../plots/patchbay/workbench/model/src/zero_body_front_door_tests.rs) assert inert opening and preservation of the joined Body identity. | Join consumes a supplied proof-bearing candidate. Its [transition](../../plots/patchbay/workbench/model/src/front_door_transition.rs) locally admits a Part; this is not by itself a remote authenticated owner exchange. |
| [Installed Birth](../../products/conduit/src/durable_host_control/body_birth.rs) | Host/Boot-bound Birth draft and acknowledged Face interaction; journals the accepted Body before publishing its owner. Multiple selected Plots currently refuse. | This service-owned Birth Face does not itself offer both Birth and Join. Reuse the installed admission path rather than treating candidate observation as authorization. |
| [Embodied Crèche Plot](../../plots/creche/main.conduit) | Checked Plot invoking `creche/prepare`; the [Std attachment](../../targets/std/src/body_execution/creche.rs) runs through ordinary Body Plan/Play, typed Fore, bounded kernel resources and explicit retirement. | The [encounter](../../make/body/src/deployment_encounter/mod.rs) explicitly prepares a different Body. Renaming this Plot to Womb would preserve the wrong semantics. |
| [Make](../../make/body/src/spore.rs) | Existing Host configuration → Profile/Build/Image and Body-bound Spore sealing; separate preparation and carrier receipts. Both prejoined and self-joining bindings exist. | Womb must select self-joining propagation, bind the actual originating Body and invitation, and never call `PreparedBodyBirth`. A description's Body ID or invitation string alone does not authorize membership. |
| [Browser Add a host](../../targets/browser/workspace/workspace-membership.mjs) | Uses current session, reviewed target configuration, Spore preparation and separate admission. [Runtime](../../targets/browser/runtime/src/workspace_runtime.rs) issues invitations against current Body state and commits admission state only after successful preparation/encoding. | Useful same-Body foundation, but these UI/runtime request handlers are not an ordinary Womb Plot. Reuse the operations behind an admitted Host Call; do not make the UI an authority owner. |
| [ConduitOS pending join](../../targets/conduitos/src/front_door/joining.rs) | Pending Face separates observed invitation, verified receipt and installed membership. Installation checks Body/Host/Boot correspondence. [Boot exchange](../../targets/conduitos/src/native_boot_join.rs) keeps refusal distinct. | Select one supported target for a retained Womb → launch → authenticated admission proof. Existing source and past platform receipts do not establish that new chain. |

The [Crèche preparation suite](../../tools/xtask/src/suites/creche_preparation.rs)
covers Source/codec and transactional-kernel contracts, Linux supervision, and
product Source registration. Its passing result would not establish ordinary
Womb execution or a target join. No Womb source exists in this inspected tree.

## Contract conflict and ownership

[#6458](https://github.com/dancxjo/conduit/issues/6458) and
[#6466](https://github.com/dancxjo/conduit/issues/6466) explicitly describe
preparing another Body from an existing Body. That is independent reproduction,
not propagation. Preserve their implementation and historical evidence until an
explicit replacement/handoff; do not silently redefine `creche/prepare@1`.
Open [#6837](https://github.com/dancxjo/conduit/pull/6837) overlaps lifecycle,
Source preparation and membership integration. Its branch is not evidence of
landed behavior and must not be absorbed wholesale.
The remote branch comparison specifically overlaps `creche/spore.rs`,
`workspace_runtime.rs` and `make/body/src/deployment_runtime.rs`. The local
browser extraction is preparatory work only; publishing an overlapping
implementation requires reconciliation or a named handoff.

That branch already contains `make/body/src/joining_host.rs` and its tests:
`check_joining_host` validates biography, invitation freshness, same-Body binding,
non-fulfilled lifecycle and exact checked configuration, then builds a one-Host
self-joining description without Birth or membership. It also contains joining
package/production/release modules and a browser call site. These are existing
unmerged foundations to review for a narrow handoff, not missing functionality
to independently recreate. No proof on that branch has been rerun by this audit.

The user authorized coordination with the owning **Greenhouse Body** chat.
Its owner handed off the narrow shared construction and browser proof from
`9a61d38899` and `dcf128253d`, assigning fresh qualification here. The local
adaptation preserves the extracted browser preparation module; it does not
import the omnibus branch, package/ZIP production, live services or MCU work.
The broader module overlap and the Integration gate still apply.

The user's requested Womb semantics are the scope for this work. They do not
authorize changing existing Kind identity semantics under the same revision.
No second scheduler, deployment graph, configuration syntax or admission system
is needed.

## Smallest coherent implementation

1. Review and obtain the existing same-Body preparation contract in `make/body`
   through a narrow handoff: consume existing
   checked Host configuration and current Body/invitation evidence, require one
   self-joining Host, and retain existing artifact/carrier provenance. Refuse
   mismatched Body, stale invitation, prejoined assignment and unsupported
   target before effects. Do not mint a Body, Boot, Part or membership.
2. Add ordinary `plots/womb/main.conduit` with a distinct semantic operation for
   propagation. Reuse the transactional request/completion mechanism while
   preserving distinct Kind identity and exact admitted operation binding.
   Request references identify owner state; possession is not authority.
3. Attach that operation to the existing Body runner and selected Host backend.
   Reuse `HostConfigurationEncounter` and make package selection. Retain actual
   producing Body/Plan/Play and current acknowledged Show checks. Expose progress,
   typed refusal and cancellation through the ordinary Face. Retire platform
   effects before kernel ownership; pressure must never repeat preparation.
4. Prove one additional Host with the existing invitation/admission machinery.
   Prefer a supported hosted/browser target before firmware. Assert unchanged
   originating Body, unchanged membership after artifact preparation, and exactly
   one new admitted Part only after authenticated admission. Retain artifact,
   launch and membership outcomes separately.
5. Extend supported unbodied encounters to explicitly offer Birth or Join,
   preserving their existing authenticated admission owners. Move or rename the
   independent reproduction encounter only with its exact contract and ownership
   accounted for; it must not remain Womb's implicit first step.

The selected first target is a second local browser Host. Initial ownership is
this audit and the browser same-Body preparation seam/tests in
`targets/browser/runtime/src/creche/spore/workspace_browser/` and its parent
module. Extract that existing operation without changing its semantics before
adding an ordinary Plot binding. The subsequent make contract belongs in
`make/body`.
Existing shared lifecycle and native Mask files need an overlap check before
extension. Root manifests, STATUS, release automation, hardware deployment and
independent reproduction are outside that first slice.

## Required evidence

Deterministic contracts must cover same-Body binding, no premature membership,
wrong/stale invitation, denied admission, unsupported target, unavailable
builder, cancellation and pressure. Kernel execution must prove the checked
Womb uses the ordinary Plan/Play and admitted Host Call. A real supported
target launch and authenticated owner exchange must separately establish the
end-to-end slice. Browser, hosted, emulator, physical and human evidence retain
their own proof classes. Full acceptance remains open until these runs exist.

## Integration gate

The original inspected source's [Integration run](https://github.com/dancxjo/conduit/actions/runs/43547912460)
failed in `proof / target / raspberry-pi`: compiling the ARMv6 image reports
E0433 at `targets/conduitos/src/arch/armv6/mod.rs:123`, where `mailbox::call`
is referenced while the module declaration is gated on `armv6-product`.
That file overlaps open [#6862](https://github.com/dancxjo/conduit/pull/6862).
The existing integration owner must resolve or hand off that failure before
feature admission; it is not part of Womb's browser implementation scope.
The repair has since merged as [#6907](https://github.com/dancxjo/conduit/pull/6907),
and this work was refreshed onto `dev=5d4116ad98`. Its containing Integration
was queued at inspection; merged repair is not a release-acceptance claim.

## Local prerequisite qualification

The browser preparation function was first extracted unchanged to
`creche/spore/workspace_browser.rs`, preserving its existing public path.
A body comparison permits only module-path and formatting changes.
Two new deterministic tests verify unchanged origin biography/membership and
exact self-joining Spore binding, and refusal of another Body's invitation
before configuration/make. They use an illustrative selected image digest;
they do not manufacture or acquire a browser distribution.

Focused browser runtime Crèche tests: **68 passed**. Full browser runtime
library: **500 passed, 5 ignored**, with no failures. Formatting, patch hygiene
and audit relative links pass. Raw local logs stay under ignored
`target/proof/creche-womb/`. These are CPU contract tests, not Chromium,
live transport, authenticated new-Host admission or Womb Plan/Play proof.

The handed-off checker is now adapted locally. Browser preparation uses the
same existing-Body construction, original Birth sign and exact Host pins, with
explicit preparation-time freshness checking. New regression coverage also
rejects stale/expired claims without changing origin biography. Browser target
construction and its transactional ABI handler were extracted so their parent
files shrink. Public claim inspection does not establish unused invitation
state, owner authority or admitted membership; the admission owner still owns
those decisions. A future Womb result must preserve typed refusal categories;
the current browser preparation API still groups make failures as `Make.Prepare`.
Fresh qualification of this adaptation: **59 make/body library tests passed**,
and **501 browser runtime library tests passed, 5 ignored**. No failures.
Formatting, patch hygiene and relative links also pass.

The first cold build was stopped before tests to address disk pressure. Only
this worktree's regenerable `target/debug` was removed after compilers stopped;
source and `target/proof` were preserved. Free space rose from 8.8 GB to 26 GB,
and was 23 GB after rebuilding without debug information or incremental caches.

## Ordinary Womb contract

Local [Womb Source](../../plots/womb/main.conduit) now checks and expands with a
distinct `womb/prepare` Kind, request/outcome types and Host Call identity.
Its Back and Crèche's Back share an extracted finite digest-call transaction;
their semantic contracts, completion codecs and exact eligibility remain
distinct. No new scheduler, configuration language, artifact pipeline or
builder offer is introduced.

The new kernel tests execute each classified outcome, retain unaccepted output
without repeating effects, reject another request's completion, and keep
mechanism denial/failure/cancellation distinct. Measured Steps allocate,
reallocate and deallocate zero bytes/objects. The checked Source cannot use a
Crèche-equipped Host as an implicit Womb builder. The existing eight Crèche
kernel tests also pass, including pressure, hard cancellation and late completion.
All 59 make/body library tests and 18 focused installed Std Crèche tests pass.
The preparation suite passes, including
both Source/kernel protocols, eight Linux builder regressions and the existing
product Source test. Strict make/body library/test Clippy and the evidence
retention guard also pass. Its canonical entrance is now
`cargo xtask check crib-preparation`; `creche-preparation` remains a thin CLI
alias for retained instructions and proof. Current UI naming migration is
coordinated with the relevant owners.
The explicit Plot inventory and its checker catalogue now include Womb.
`cargo xtask check plots check` passes all **111 canonical Plots**, and
`cargo xtask check plots run --deterministic --plot womb` passes its registered
Source oracle. Womb is excluded from the Workspace shelf until installed
execution support exists; no availability or Plan/Play claim is inferred.
These are checked Source and deterministic production-kernel proofs with
fixture evidence, not an installed Womb Body Play or artifact/join chain.

The product Source checker registration is reserved with the active
**Greenhouse Body (2)** owner through the authorized parent handoff. Its minimal
patch is retained locally in ignored `target/proof/creche-womb/`; this chat has
not changed the owned `products/conduit/src/plot_source.rs`. Installed builder
ownership, ordinary Face progress and the authenticated browser-target slice
remain unfinished.
The first-join slice does not establish Seed issuer continuity across owner
restarts. Successor authority remains a separate concern under
[#6409](https://github.com/dancxjo/conduit/issues/6409) and
[#6340](https://github.com/dancxjo/conduit/issues/6340); changing a contact or
owner pin is not an authority handoff.

## Verified release binding handoff

The isolated `joining_host_release` module reuses the owner-authorized
`bd4d056f31` implementation and `b958243519` module tests, adapting the shared
joining-Host refusal to its existing display boundary. No packaging, product
or browser differential-suite changes from those checkpoints are imported.
The binder accepts the existing opaque `VerifiedHostReleaseBytes` observation,
checks its exact source, target, Make package and adapters against the selected
IMAGE, and binds the verified inventory aggregate to the existing Body's
original Birth, checked configuration and exact invitation. A raw payload SHA
is not substituted for the aggregate. Invitation expiry is rechecked.

The three deterministic tests use a deliberately non-executable payload. They
prove the binding and refusal of source/target/package/adapter substitutions,
expired invitations and changed checked configuration, while preserving the
Body evidence. They do not prove a runnable browser bundle, immutable byte
custody after observation, packaging, deployment, Boot or admission. The
packaging owner must retain/reverify the actual bytes and target-specific
IMAGE validation remains mandatory. `cargo xtask check crib-preparation`
includes these tests and the typed joining-Host refusal tests.

## Private package production handoff

The producer-only extraction of `joining_host_package` comes from the handed-off
`4d3fbc71504ef6eb93a2afb96d3090ddc5bcc8f1` checkpoint, including its safety
follow-ups rather than the unsafe original ZIP introduction. It uses the
existing stored-ZIP carrier and current browser reader. The exact Spore,
canonical Host source, IMAGE, release provenance and portable invitation are
retained separately. Every payload is checked before and during writing;
finite headers, provision and complete archive size are admitted first.
Cancellation or changed/short/extra payload bytes yield no completion receipt.
The caller must keep output private and discard partial output on refusal.

Only the producer, its common fixture and ten producer tests are extracted.
The downstream Rust package reader and its test require Host IMAGE validation
and a ZIP-parser seam absent from this baseline; they remain with their source
owner rather than pulling those independently owned contracts into this lane.
The UF2 packaging fixture uses current `pico-w.host.conduit`, replacing the
checkpoint's absent house-specific profile. Synthetic payloads prove byte and
format correspondence, including the existing JS reader, not executable target
validation, deployment, Boot, admission or Womb Body execution. Target-specific
validation and issuer custody remain required before a member Seed is offered.
