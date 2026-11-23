# Sprint Archibold: first usable Body, then wider Hosts

**Current sprint policy, revised 20 November 2026.** This is a product goal, not a claim of implementation or release acceptance. [#6984](https://github.com/dancxjo/conduit/issues/6984) owns the product acceptance and [#6983](https://github.com/dancxjo/conduit/issues/6983) owns the Now/Next/Later board index. Historical broader scope and storage decisions remain in those issues and in the git history of this document.

> **First deliverable:** download and launch a supported Conduit product, enter Crib, choose useful resident Plots, create a Body, use the Ambient Commons and Patchbay, leave, and return to **the same Body** through a Host that already has a proven persistence mechanism.

**Do not make a useful first vertical slice depend on bare-metal ConduitOS microSD, QEMU writable block support, Pi3B+ firmware bring-up, or a new general storage system.** Those are independent expansion paths. Preserve honest refusals and do not claim cross-boot retention where no durable Back exists.

## NOW: complete one first-user journey

| Order | Existing owner | Small observable outcome |
|---|---|---|
| 1. Crib and first Host | [#6911](https://github.com/dancxjo/conduit/issues/6911) and [#6984](https://github.com/dancxjo/conduit/issues/6984) | On a real selected current Host/Boot, Crib offers Create/Join; Birth creates exactly one Body and admits its first Part with an exact checked resident Plot selection |
| 2. Resident default experience | [#6909](https://github.com/dancxjo/conduit/issues/6909), accepted chime [#3152](https://github.com/dancxjo/conduit/issues/3152) | Clock and Ambient Face continue in the admitted Body; wallpaper/weather, audio and startup chime are selected where offered and unavailable resources do not stop the Body |
| 3. Commons and Patchbay | [#5354](https://github.com/dancxjo/conduit/issues/5354), [#5363](https://github.com/dancxjo/conduit/issues/5363), [#6201](https://github.com/dancxjo/conduit/issues/6201), open [PR #6990](https://github.com/dancxjo/conduit/pull/6990) | Real graphical Commons opens at Ambient, current clock/image/weather status and navigation to usable Patchbay through one checked Face/Mask/Show and exact source-bound interactions |
| 4. Return to the Body | [#6984](https://github.com/dancxjo/conduit/issues/6984) | First prove a browser retained-profile close/reload, then an installed **hosted Linux** Host service retaining its Body across closing the UI and a genuine reboot; re-entering doesn't silently Birth another Body |

The first product can expose a **smaller honest default** when no weather, wallpaper, audio or graphics provider is available. A clock alone plus accessible navigation is useful. Startup Chime has a **Body-Wake** contract, not an assumed every-Host-Boot sound. Civil-minute clock advancement should use admitted time basis and local qualified deadlines, not repeated weather fetches or application Play restarts.

**First vertical acceptance (independently report browser and installed Linux):**

- [ ] Reviewed runnable user entrance without Cargo, source checkout or compiler; identify the exact released artifact and Host.
- [ ] Crib makes or explicitly Joins one genuine Body, with current Host/Boot/Part evidence and checked, user-selectable resident work; no fake membership from Seed/rendezvous receipt alone.
- [ ] Same authoritative Face and actions are accessible via actual acknowledged graphical/linear presentation, with Ambient → Patchbay usable by keyboard and applicable pointer controls.
- [ ] Missing/expired weather, unavailable image, unsupported sound or stale input fails honestly but does not block otherwise admitted work.
- [ ] Closing and reopening the browser app on retained storage returns to the same Body; separately, closing hosted-Linux UI preserves a configured durable Host, and fresh supported service Boot restores the same BodyId with fresh BootId, Plan and Play.
- [ ] Evidence is truthful for its exact proof class: source/test, Chromium, installed Linux, emulator, physical and released artifact must not be conflated.

This milestone is **not blocked by a second browser member, complete Look language, cross-platform installer parity, native ConduitOS persistence or any Pi microSD implementation**.

## NEXT: expand the working experience

1. **Stem and same-Body member propagation:** [#6911](https://github.com/dancxjo/conduit/issues/6911) remains the owner for the canonical name **Stem** (formerly Womb) as a real **Crib-catalog-selectable resident Plot**, not a privileged special provisioning UI. Prove checked catalog entry, selection into Body Plan/Play, an ordinary Face/Mask/Show encounter, selected effectful Back, invitation-bound Seed preparation, and a second authenticated **browser Host** admitted into the original Body. This follows the first Body's usable experience rather than blocking it.
2. **Windows/macOS hosted entrance and Return:** qualify verified target artifacts, real installation/startup constraints, returning to Body on the **same retained Host identity**, and truthful service absence, independently. Reuse completed release foundations [#2096](https://github.com/dancxjo/conduit/issues/2096) and installed-host precedent [#3417](https://github.com/dancxjo/conduit/issues/3417); do not claim ZIP packaging equals an installer.
3. **Cross-Mask fidelity:** [#6916](https://github.com/dancxjo/conduit/issues/6916) ensures wallpaper/time subjects remain accessible in linear/spoken encounters without unsolicited minute/rotation narration. Maintain original Face actions and current Show acknowledgement.
4. **ConduitOS x86_64 first use:** separately qualify a normal **single-Boot** Crib/Commons/Patchbay on actual QEMU. It may be intentionally **ephemeral** if native writable storage is absent; mark standalone cross-boot recovery unsupported, and distinguish optional **remote Join of a Body kept by another durable Host** from local persistence.

Do not hold a completed browser/Linux journey hostage for later target acceptance. Record each new target on its own matrix row and promote exact accepted artifacts through the normal CI and publisher.

## LATER: independent disk and physical Pi tracks

- [#6859](https://github.com/dancxjo/conduit/issues/6859): physical **Threebee, Raspberry Pi 3 Model B+ rev 1.3**, direct-firmware **AArch64 ConduitOS** boot, graphics, serial and input. This is **not** Raspberry Pi OS on the Pi3B+ ([#6325](https://github.com/dancxjo/conduit/issues/6325)) or the older ARMv6 Model B+ (reportedly damaged).
- [#6996](https://github.com/dancxjo/conduit/issues/6996): *after actual board readiness and an explicit hardware handoff*, inspect the correct Threebee SD controller and implement an admitted, bounded microSD block read/write/flush Base, safe dedicated DATA/SOUL region, committed Body continuity and power-interruption recovery. Preserve **same BodyId/HostId, fresh BootId/Plan/Play**. Never write an inhabited physical card automatically; use reviewed disposable media.
- Optional ConduitOS **x86_64 QEMU durable storage** is a separate Back and acceptance path from physical Pi microSD. Historical [#3960](https://github.com/dancxjo/conduit/issues/3960) is an architectural proposal, **not** installed recovery proof. Both tracks can use common checked DATA/DISK/SOUL meaning without embedding a filesystem or animation engine in the kernel.
- Deferred unless proven necessary to the first product: generalized Control Panel [#6982](https://github.com/dancxjo/conduit/issues/6982); complete Look syntax [#5377](https://github.com/dancxjo/conduit/issues/5377); reactive Look interpolation, ambient sensor and PWM [#6994](https://github.com/dancxjo/conduit/issues/6994); full input-control vocabulary [#6038](https://github.com/dancxjo/conduit/issues/6038); general writable namespace/filesystem [#6442](https://github.com/dancxjo/conduit/issues/6442)/[#6443](https://github.com/dancxjo/conduit/issues/6443).

**Do not confuse a target-specific storage acceptance failure with an upstream prerequisite to a different Host's working experience.** Native ConduitOS can show useful meaning in one Boot without pretending to be durable. Hosted Linux and browser already provide narrower paths for persistence; qualify their actual Body continuity rather than taking it on trust.

## Current priority and delivery rules

**P1/core:** #6984 product acceptance; #6911 Crib first Body; #6909 default ambient; #5354/#5363/#6201 Commons and usable Patchbay.

**P2/deferred independent hardware:** #6859 Threebee ConduitOS Boot and #6996 Threebee microSD Body continuity. #6994 advanced reactive Look remains deferred. Lower priority does not close or weaken any physical contract.

Work in small independently reviewable PRs with explicit owner/hand-off and concrete code, contract and equipment ownership. Allow independent **Crib/distribution**, **ambient/native**, and **Commons/Face/Mask** work in parallel as practical, with one integration shepherd. No arbitrary global two-active-PR limit once [the contributor instruction PR #6995](https://github.com/dancxjo/conduit/pull/6995) merges; protect overlapping shared-core writers, current branches, and physical media. If `dev` is red, repair/revert before more feature admission.

Every handoff records: owning issue and first-slice acceptance criterion, inspected current `dev`, exact branch/head, modified files/contracts, tests and proof class, failures/blockers, and next named owner. **Do not create an architecture prerequisite without demonstrating which first-use test fails.** Keep candidate, exhaustive integration, source-matched release promotion and published-site receipts distinct.

## Acceptance matrix, not one global gate

| Target | First-use goal | Return expectation |
|---|---|---|
| Browser | Reviewed static/packaged Crib → Body → Ambient Commons/Patchbay | Same retained Body on reload/return within the eligible browser origin/profile |
| Hosted Linux | Reviewed install and actual durable Host service → same Commons | Same HostId/BodyId after GUI exit and supported reboot, with new BootId/Plan/Play |
| Hosted Windows | Reviewed native artifact and explicit supported launch | Independently verify durable re-entry mechanism or disclose absent service |
| Hosted macOS Apple silicon | Reviewed native artifact and explicit supported launch | Independently verify durable re-entry mechanism or disclose absent service |
| ConduitOS x86_64 QEMU | Actual native **single Boot** Crib/Commons/Patchbay | No local cold-reboot claim without an admitted durable disk; remote Join separately qualified |
| ConduitOS AArch64 Threebee Pi3B+ | Exact physical bare-metal Boot after #6859; real input/presentation where offered | **Deferred #6996:** independently written/recovered microSD SOUL on physical cold reboot, not a blocker for browser/Linux |

**Archibold makes good progress by finishing the first row and first installed Host row, not by requiring all six rows to become green before anyone can use it.** The wider project can continue qualifying the other rows one by one.
