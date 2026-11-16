# Narrow same-Body Host preparation handoff — 2026-11-15

For the Crèche/Womb owner, thread `01a1e2a3-f527-7600-868a-ce76cf949a01`, branch `codex/creche-womb`. Read-only audit of `work/environment-extraction-integration-20261111`, HEAD `0e7bfb3d9a`, against the requested `origin/dev` snapshot `4400d3dffc`. No builds, code changes, device operations or new qualifications were performed.

## Smallest useful dependency-closed delta

**Take only `9a61d38899` plus `dcf128253d`, adapted into your existing spore/workspace_browser.rs extraction.** This is the existing same-Body browser-preparation seam; package production, release sealing and ZIP writing are additional independent layers, not prerequisites for this seam. Do not cherry-pick the omnibus integration branch.

`9a61d38899` (“share existing-Body Host construction with browser Seed preparation”) changes exactly:

* `make/body/src/joining_host.rs` — new `JoiningHostSelection` and `check_joining_host`.
* `make/body/src/joining_host/tests.rs` — original Birth/checked configuration and wrong Body/expiry/corrupt biography tests.
* `make/body/src/lib.rs` — only `mod joining_host` and its public re-export for this slice.
* `targets/browser/runtime/src/creche/spore.rs` — `prepare_workspace_browser` calls shared `check_joining_host` rather than reconstructing an ad hoc browser Body description; takes `now_millis`.
* `targets/browser/runtime/src/creche/spore_target.rs` — names the already-existing reviewed browser release Source identity as `BROWSER_RELEASE_SOURCE_ID`.
* `targets/browser/runtime/src/workspace_runtime.rs` — forwards the existing request's `now_millis`.

`dcf128253d` adds the 64-line `workspace_browser_seed_uses_shared_checked_host_and_original_birth` test to `targets/browser/runtime/src/creche/spore_tests.rs`. This is the focused browser integration proof to transplant into your new module/test arrangement.

At `4400d3dffc`, `SelectedPrebuiltContent` and `seal_prebuilt_body_spore_with_content_digest` already exist in `make/body/src/spore.rs`, and `VerifiedHostReleaseBytes` exists in `make/host/src/release_bundle.rs`. The minimal pair does not require later release-bundle changes, MCU changes, authoring-encounter changes, or the prepared Source checker lane. There were no subsequent commits modifying the minimal helper/browser adaptation in the audited range except the added test.

## Contract preserved

`check_joining_host` validates biography, current invitation, matching Body identity and nonfulfilled Body; rechecks the exact selected Host configuration under the supplied catalogue/packages; and retains the original Body identity and public invitation. It explicitly does **not** issue invitations, provision secrets, produce artifacts, deploy, observe Boot or admit members. Browser preparation additionally preserves the exact original Birth Sign and selects/seals the reviewed prebuilt browser IMAGE. The new wall-time input must be on the invitation's clock basis.

## Optional adjacent layers: do not import just to obtain browser preparation

| Layer | Introducing commits | Paths / dependencies |
| --- | --- | --- |
| Streamed reviewed-artifact production | `3eae7edd5c` | `make/body/src/joining_host_production.rs`, its `tests.rs`, two lib declarations. Depends on minimal joining helper and existing prebuilt seal. Owns bounded read/hash/cancellation, retained artifact and receipt; no deployment/admission. |
| Same-Body verified-release seal | `bd4d056f31`, tests `b958243519` | `make/body/src/joining_host_release.rs`, its `tests.rs`, lib declarations. Depends on helper and existing `VerifiedHostReleaseBytes`; binds inventory aggregate rather than ZIP/payload SHA. `b958243519` also touches `tools/xtask/src/suites/browser_check.rs`; review that separately. |
| Bounded private ZIP package | `4e631a5b1d` and later fixes | `make/body/src/joining_host_package.rs`, `reader.rs`, `make/body/tests/{joining_host_package.rs,common/joining_host_package.rs}`, lib. Broader consumer changes enter Linux native install and `targets/browser/workspace/creche-native-zip.mjs`. Not part of minimal slice. |

**Do not take the original ZIP-introduction commit alone as the current qualified implementation.** Later package-related fixes include `6fb46b7eab` (provenance), `900349bbca` (authenticated owner export), `d844b4f3d1` (Make provenance), `3a2eacd8a1` (original invitation across fresh Boot), `3683b7d147` (checked Host source/actual IMAGE), `2e37d65b49` (UF2) and `2f4ab2a2a6` (verified installation bytes). Several span unrelated native/MCU consumers. If Womb needs packaging, extract current generic paths in a separate reviewed slice and qualify against its chosen consumers; do not absorb these commits wholesale.

## Existing evidence and remaining gates

Existing tests cover helper identity/refusal, browser original Birth/shared construction, production changed/truncated/extra bytes, expiry after I/O, cancellation, source/configuration/manifest substitution, finite bounds, and release aggregate identity/substitution. Package integration tests also exercise the existing JS reader. These are present source-level proofs, not a fresh successful run on `codex/creche-womb`.

Historical recorded qualification: `make/body/src/host_configuration_encounter/qualification-20261101.json` records six passing `joining_host_production::tests`, five encounter tests and body-make lib clippy on its explicitly recorded older combined source. Its unrelated authoring commits and qualification file are not required dependencies of the minimal helper. Do not represent that historical record as qualification of today's extraction.

Before delivery, run the focused helper tests and the transplanted browser preparation test against your branch; then the existing fixed-count browser proof contract (your 68 focused tests) and applicable Candidate checks. Demonstrate that current `now_millis` reaches the helper after your split. A sealed Seed remains preparation evidence, not successful admission. Live deployment, private invitation provisioning, fresh Boot, and acknowledged same-Body membership remain independent gates.

## Ownership / overlap

The active MCU lane is not changing this browser seam. Your `spore/workspace_browser.rs` extraction directly overlaps the **function relocation and test relocation** from the minimal pair, so port their semantics rather than reintroducing `spore.rs` or overwriting your tests. Shared `make/body/src/lib.rs` edits must be limited to the helper's two declarations; current integration lib also exports optional production/release/package/ZIP modules that you need not take. Nothing in this handoff requires changing environmental application code, the kernel, or MCU runtime.
