# Crèche and Womb: implementation gap

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

The inspected source's [Integration run](https://github.com/dancxjo/conduit/actions/runs/43547912460)
failed in `proof / target / raspberry-pi`: compiling the ARMv6 image reports
E0433 at `targets/conduitos/src/arch/armv6/mod.rs:123`, where `mailbox::call`
is referenced while the module declaration is gated on `armv6-product`.
That file overlaps open [#6862](https://github.com/dancxjo/conduit/pull/6862).
The existing integration owner must resolve or hand off that failure before
feature admission; it is not part of Womb's browser implementation scope.

## Local prerequisite qualification

The browser preparation function was extracted unchanged to
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

The first cold build was stopped before tests to address disk pressure. Only
this worktree's regenerable `target/debug` was removed after compilers stopped;
source and `target/proof` were preserved. Free space rose from 8.8 GB to 26 GB,
and was 23 GB after rebuilding without debug information or incremental caches.
