"""Preserve explicit MCU source and public qualification evidence without moving checkout/index."""
from pathlib import Path
import os, subprocess, tempfile, json
root=Path(__file__).resolve().parent.parent
repo=root/'work/environment-extraction-integration-20261111'
branch='checkpoint/greenhouse-source-preparation-20261114'
base='docs/checkpoints/greenhouse-source-preparation-20261114'
def run(*a,**kw):return subprocess.check_output(['git',*a],cwd=repo,**kw)
old=run('rev-parse',branch).decode().strip();head=run('rev-parse','HEAD').decode().strip()
fd,index=tempfile.mkstemp(prefix='conduit-checkpoint-index-');os.close(fd);os.unlink(index)
env=dict(os.environ,GIT_INDEX_FILE=index)
def git(*a,input=None):return run(*a,input=input,env=env).decode().strip()
def put(path,data):
 blob=git('hash-object','-w','--stdin',input=data)
 git('update-index','--add','--cacheinfo',f'100644,{blob},{base}/{path}')
try:
 git('read-tree',old)
 paths=set(git('diff','--name-only',old,head).splitlines())|set(git('diff','--name-only','HEAD').splitlines())|set(git('ls-files','--others','--exclude-standard').splitlines())
 paths=sorted(p for p in paths if p=='Cargo.lock' or p.startswith(('targets/mcu/','targets/rp2040/','targets/conduitos/plots/usb/','architecture/')))
 for p in paths:
  if (repo/p).is_file():git('add','--',p)
 names=['positive-shared-stock-factory-20261114.log','positive-shared-stock-factory-20261114.json','rp2040-shared-stock-build-receipt-20261114.json','rp2040-shared-stock-footprint-20261114.json','rp2040-shared-stock-link-20261114.log','rp2040-shared-stock-inference-failure-20261114.log','shared-stock-runtime-tests-20261114.log','shared-stock-runtime-clippy-20261114.log','shared-stock-topology-tests-20261114.log','shared-stock-topology-clippy-20261114.log','prepared-owner-retention-audit-20261114.md','kernel-immutable-storage-audit-20261114.md','kernel-linked-terminal-qualification-20261114.json','kernel-linked-terminal-20261114.patch','kernel-linked-terminal-compile-20261114.log','kernel-linked-terminal-tests-20261114.log','kernel-linked-terminal-all-tests-20261114.log','kernel-linked-terminal-noalloc-check-20261114.log','kernel-linked-terminal-clippy-20261114.log','mcu-linked-terminal-tests-20261114.log','mcu-linked-terminal-clippy-20261114.log','positive-linked-terminal-factory-20261114.log','positive-linked-terminal-factory-20261114.json','rp2040-linked-terminal-build-receipt-20261114.json','rp2040-linked-terminal-footprint-20261114.json','rp2040-linked-terminal-link-20261114.log','arm-linked-terminal-geometry-20261114.json','kernel-terminal-value-capacity-audit-20261114.md','arm-kernel-fields-baseline-20261114.json','kernel-phase-terminal-qualification-20261114.json','kernel-phase-terminal-20261114.patch','kernel-phase-terminal-compile-20261114.log','kernel-phase-terminal-tests-20261114.log','kernel-phase-terminal-all-tests-20261114.log','kernel-phase-terminal-noalloc-check-20261114.log','kernel-phase-terminal-clippy-20261114.log','greenhouse-work-accounting-20261114.md','mcu-phase-terminal-tests-20261114.log','positive-phase-terminal-factory-20261114.log','positive-phase-terminal-factory-20261114.json','source76-execution-handoff-audit-20261114.md','rp2040-phase-terminal-build-receipt-20261114.json','rp2040-phase-terminal-footprint-20261114.json','rp2040-phase-terminal-link-20261114.log','arm-phase-terminal-geometry-20261114.json','source76-ARM-heap-lower-bound-20261114.json','selected-stock-diagnostic-20261114.log','local-fore-zero-remote-trace-tests-20261114.log','local-fore-zero-remote-trace-clippy-20261114.log','positive-local-trace-factory-20261114.log','positive-local-trace-factory-20261114.json','source-fanout-opportunity-20261114.json','source-fanout-opportunity-20261114.md']
 names += sorted(p.name for p in (root/'outputs').iterdir() if p.is_file() and p.suffix in ('.md','.json','.log') and ('20261115' in p.name) and p.name.startswith(('kernel-unresolved','source-unresolved','mcu-unresolved','unresolved-adapter','rp2040-unresolved','arm-unresolved','positive-unresolved','fanout-proof-review','fanout-custody-issue','creche-womb-handoff','native-profile','native-runner','profile-runner','pr6837-resume','greenhouse-resume','green-readonly-status','green-root-inspection','yellow-root-inspection','blue-root-inspection','blue-root-continuity','green-recovery-challenge','source76-ram-partition-audit','cord-terminal-stock-audit','neutral-execution-source76','neutral-execution-cord-terminals','neutral-execution-display-independence','cord-terminal','source-cord-terminal','positive-cord-terminal','arm-cord-terminal','arm-software-preparation','rp2040-cord-terminal','owner-return-continuity','usb-return-owner','history-capacity-recovery','blue-reviewed-application-readiness','blue-operator-reseed-preflight','current-root-public-carrier-receiver','inactive-incremental-cleanup','environment-maintenance','neutral-execution-production','neutral-production','cache-preservation-headroom','positive-cold-factory','neutral-execution-cold-factory','cold-factory-source','rp2040-cold-factory')))
 for name in names:
  p=root/'outputs'/name
  if p.is_file():put('evidence/'+name,p.read_bytes())
 for name in ['run-rp2040-shared-stock-build-20261114.py','run-rp2040-linked-terminal-build-20261114.py','run-rp2040-phase-terminal-build-20261114.py','audit-source-fanout-20261114.py','run-rp2040-unresolved-stock-build-20261115.py','measure-unresolved-geometry-20261115.py','measure-unresolved-factory-20261115.py','qualify-unresolved-adapters-20261115.py','query-connected-roots-20261115.py','monitor-connected-root-20261115.py','run-rp2040-cord-terminal-build-20261115.py','run-rp2040-cord-terminal-llvm-build-20261115.py','rp2040-source-fit-build.py','measure-cord-terminal-geometry-20261115.py','measure-cord-terminal-factory-20261115.py','qualify-cord-terminal-source-20261115.py','run-rp2040-cold-factory-build-20261115.py','measure-cold-factory-20261115.py','qualify-cold-factory-source-20261115.py','preserve-source-checkpoint-20261114.py']:
  put('scripts/'+name,(root/'work'/name).read_bytes())
 for name in ['command.json','type-sizes.log']:
  put('scripts/arm-kernel-fields-baseline/'+name,(root/'work/arm-kernel-fields-20261114'/name).read_bytes())
 for name in ['command.json','probe.rs','compiler.log','sizes.json']:
  put('scripts/arm-preparation-minimum/'+name,(root/'work/arm-preparation-minimum-20261114'/name).read_bytes())
 for name in ['command.json','probe.rs','compiler.log']:
  put('scripts/arm-cord-terminal-geometry/'+name,(root/'work/arm-cord-terminal-geometry-20261115'/name).read_bytes())
 for name in ['command.json','probe.rs','compiler.log']:
  put('scripts/arm-software-preparation-geometry/'+name,(root/'work/arm-software-preparation-geometry-20261115'/name).read_bytes())
 put('scripts/thumb-stack-frames.py',(root/'work/factory-tools/thumb-stack-frames.py').read_bytes())
 for name in ['command.json','probe.rs','compiler.log']:
  put('scripts/arm-unresolved-stock-geometry/'+name,(root/'work/arm-unresolved-stock-geometry-20261115'/name).read_bytes())
 for name in ['command.json','probe.rs','compiler.log']:
  put('scripts/arm-phase-terminal-geometry/'+name,(root/'work/arm-phase-terminal-geometry-20261114'/name).read_bytes())
 for name in ['command.json','probe.rs','compiler.log']:
  put('scripts/arm-linked-terminal-geometry/'+name,(root/'work/arm-linked-terminal-geometry-20261114'/name).read_bytes())
 note=f"""# Continue the greenhouse MCU work

Shared qualified source parent: {head}. This recovery commit preserves all current experimental MCU source byte-for-byte alongside public evidence. Qualified shared changes are in PR6837; this draft PR6902 is not a deployable release.

## Production execution handoff — 2026-11-15

New neutral_execution module is included by the board and original CPU/no_std fixtures. It safely checks complete Fore pins and result backing cold, uses original prepared service custody and factory/admission/Start, samples time again at Start and never clones the Root advertisement. Actual serial service still refuses missing installed Fore/Mask. Final updated two-scenario/no_std qualification PASSED (2 tests,56.37s) with five cold refusal cases and repeated Start refusal. Strict scoped Clippy passed. Read neutral-production receipts for exact bounds; CPU proof remains distinct from physical target fit. No hardware update.

Blue operator-reseed preflight confirms same Body and retained original Part/key, but current credential differs old103basis and current Root peer differs old Seed pin. Existing reviewed application matches archived install readbacks; current flash not re-read. No reseed done. History audit confirms read-only official service/provenance; no qualified erase/generation reclamation.

## Latest cold-phase stack qualification — 2026-11-15

The no-inline factory phase fence passes original factory/no_std and authenticated Source76 execution tests. Exact ARM ELF5ee7d2fbac0725740ed36fe82b1d82d68601b7f09d38ffe8f3cf41a8dc073fc6 reduces preflight frame26960 to15480. Largest identified direct-call path52840 versus historical64320; region61248 unchanged. This is NOT a complete stack upper bound: indirect calls, dynamic paths and interrupts remain unqualified. No hardware deployment or heap fit claim. Older stack-gate evidence below predates this change.

## Latest Cord-terminal stock + real Source execution — 2026-11-15

QualifiedsharedCordstock defaultsfullC; compactexactalllogicalRemote->Localcount onlywithoutAbnormalTerminaltracks. Source76 uses3, not0. Full100byteingressterminals/localFore/replay/cancelinspection/rearmtruth remain. Kernel271allfeaturetests/3ignored,5localForetests,noalloccheck/strictallfeaturelibtestsClippy PASS. Runtime140+21tests/strictClippy PASS; Source7tests/strictClippy PASS. ActualSource76wholeadmission/Start/nativeRunner/numerical54transactionproof repeatedPASS withnewstock3. Originalfactory/Root/no_std PASS CPUretained141533/peak143572, additional9576 saved/combined68821fromoriginal. ActualARMkernel21968 versus31536previous(total9568saving;9384versusnewfullstockrepresentation31352). FullprepentranceELFc28b24b9da97080a6ef43926dd3c532c1e3a11255defa225c81e423726bd8ba8, flashmargin49380, static209088, stack61248, heap131072unchanged.

STACK GATE: exactELFdirectcallchainshows64320>61248 (beforeindirectcalls/IRQs); notsafehardwarefit. Localpreflightframe26960; initializerinventory8832, schema/numberdecoderpathslarge. LLVM IR/cold-stackevidence savedasreceipts, originalIRregenerable53MB localonly. Nexttestcoldphaseoutlining/actualtemporaryoverlap, notgrantgrowth. No newhardwareimageflashed.

OWNER CONTINUITY GATE: standardRoot746 return-usb oldBluepublicbasis withnonexistentport deterministicallyrefusesownerpinbeforedeviceI/O/receipt/commit. CurrentRootcarrierkeynewperBoot; MCUseedpinexactoldowner. Existingmembercontinuity/recipepermissiondonotadvanceissuerpin. Preservechecks; authorizedreseedorexplicitdelegateddurableauthorityrequired, noarbitrarypinrewrite. Seeexisting6409commentandowner-return-continuity-audit. Root/Faceownercontrolsallliveupdates; latest746Body28fbstablebothPiACKs/touchproof. Do notclaimcurrentMCUfeeds.

## Actual checked Source76 execution qualification — 2026-11-15

Read neutral-execution-source76-20261115.md/log. Original factory ->49resourcewholeledgeradmission->authenticatedStart(repeatedStartrefuses)->ordinaryI2C/clockRunner PASS. All3typedinputs/all9typedoutputs+closure; frameonlyafterinitialized.54exactorderedI2Ctransactions(BME48+HT6), numerictemperature2508centidegC/pressure25767233Q24.8Pa/humidity55588Q22.10%, capturedphase14. Zeroexecutionallocationattempts; originalnativeownersRevoked/inflight0. Testfilesonly, productionserviceunchanged. CPUfixture32MiBstack+1MiBceiling isNOTtargetfunding, notperiodic/FaceMask/history/hardwareproof. Newtestsareunder targets/rp2040/make/tests/neutral_execution*. Do notskipremainingphysicalmemory/ForeMaskcomposition gates.

Fresh deviceinspection: Yellowrespondsbutoutputfragmentrefused/Maskcancelled; BlueanswersafterUARTboottextsettlesbutcoldcounters0; Greenbuild-boundrecoveryQUERYresponds82e528c351...38f6 (no rebootrequested), RootInspectunsupportedoldermembershipservice. HistoricalGreencredentialnames oldBodyd5b64...c4d7, notcurrentHamm28fb...21d3a; membershipflagisnotproof. CurrentRoot/Faceownercontrolsupgradesandmembershipwindows; coordinatebeforeReturn/admission. No newdevicefirmwareflashed.

Read source76-ram-partition-audit: remaining61248bytesarestack, notfreeheap. Do notincreasegrantsbasedonCPU. Cord-terminal-stock-audit proposes ~9.5k realstorage reduction under strict actualtopology; NOTimplementedorsavedatthischeckpoint. Preservefullterminaltruthandpostcancelinspection.

## Resumed 2026-11-15: checked routed-abnormal stock

Newest kernel U0 profile qualifies only actual topology without AbnormalTerminal Cords; default remains N full100-byte entries. Shared kernel265all-featuretests/3ignored,172no-defaulttests/3ignored, no-defaultcompile and strictClippy PASS. MCU adapter139tests/1ignored and strictlibClippy PASS; Source6tests and strictlib/testsClippy PASS. New ARM kernel31536 B vs39592, saving8056 B. Preparation entrance ELF c384f6489df37e7094e45cebf489bb24799ddd099c330d20b7d9996291f2b8ff links, flashmargin49708, static209088/stackregion61248, heap131072 unchanged. Original completefactory/Rootpreflight/no_std CPUproof alsoPASS:retained151109/peak153148, additional8056saved, combined59245from210354original. ExcludespreexistingRoot/equipment/residents/issuergrants/allocatoroverhead/stack; notMCUheapfunding. CPUharness now explicitly enables generated resident execution-workspace/schema-value-storage features rather than relying on incidental graph unification; TMPDIR onroot avoids tmpfsuserquota. Not whole application fit or deployment. Read latest20261115receipts before historical values below.

Actual native runner was found hard-coding the old full-storage aliases. Profile forwarding through the ordinary runner/native services is now qualified:140runtime library tests and21native integration tests PASS; strictallfeaturelib/testsClippy PASS. ActualQ1/M0/phase0/U0 I2C+clock execution preserves exactrequests/cancellation/retirement withzeroexecutionallocations. This is CPU provider evidence, not Source76 admission or hardware proof. Prototype still intentionally prepare/drops. No new hardware proof.

Fanout reduction remains DISABLED: source audit found fallible-Sign partial-commit reference custody gap, tracked #6906. See fanout-proof-review-20261115.md. Do not count the6616candidatebytes as savings. Crèche/Womb narrow handoff is also saved, separate from MCU ownership.

## Historical latest change: qualification boundary

The shared kernel now also includes PHASE_BYTES specialization (258 kernel tests passed/3 ignored; no_alloc and strict lib/testsClippy pass). Default remains100. The new MCU adapter forwards this dimension, and the RP2040 prototype derives0 from the exact Source terminal matrix. The shared MCU adapter now compiles and passes139 runtime tests/1ignored. The NEW RP2040 Source-derived factory now passes complete original CPU64 preparation/Root preflight and no_std compilation: retained164853/peak166892 B, additional7600 B saved, total45501 B saved from the original baseline. The phase-profile ARM release now links; exact selected kernel39592 B vs47192 B with full phase payloads. See newest phase receipts, not historical linked-only figures below. Runtime heap/stack/Start remain unproved. A bounded shared-adapter test was completed after preservation; no new target build or device changes followed.

The historical MCU/factory/ARM measurements below belong to c7e14f11e2. The newest CPU measurement is164853/166892 B in positive-phase-terminal-factory-20261114.json. Newest ARM geometry is39592 B kernel, preparation-entry ELF557cf6092cec42178b3280746da51fb960394669bbcc841a77e192496df3e4cb, flashmargin49636 B/static209088 B/stackregion61248 B; no physical runtime proof. Do not extrapolate the tiny phase test's104-byte saving to Source76 or ARM. Continue actual target heap/stack qualification and the execution-handoff integration; do not repeat unchanged successful CPU/link checks without a new change or concern. Public abnormal values, CordState and unresolved abnormal truth remain100-byte capacity.

## Last complete verified preparation (c7e14f11e2)

The uninstrumented original factory and original no_std modules prepare the exact signed Source76 component through the existing Root entrance:76 Gears,95 Cords,97 queue slots;27 software,47 resident,2 native owners. Assignment30314 B fits the unchanged32548 B input. All native loans return and native calls stay0. No admission/Start occurs.

Physical schema buffers are selected only after inspecting actual prepared owners and exact emitted topology. All logical slots, generation horizons, byte quotas, queue capacities and canonical/native-result extents remain. Unknown custody keeps original physical stock. Retained state, simultaneous Step outputs and kernel native pins are separate stock terms.

The same ordinary kernel now supports linked immutable terminal contracts. TERMINAL_ROWS defaults to NODES for the fixed/no_alloc profile; M=0 retains the exact checked static matrix instead of copying it. Actual Back equality, inactive rows, native incompatibility, topology, projected recovery and storage-budget gates remain. Changed by-value rebind refuses; same-content rebind remains valid. Default inline profile pays a small storage handle, charged through actual sizeSelf.

CPU64 requested retained/peak allocations:172453/174492 B versus189469/191508 with shared buffers alone and210354/212393 originally. Combined saving37901 B. These exclude preexisting Root/equipment/residents/issuer grants, allocator overhead and stack. The separately funded1MiB CPU arena is NOT target funding; installed heap remains131072 B.

Kernel255testsPASS/3ignored; focused linked-profile5PASS; no_alloccheck and strict kernel lib/testsClippyPASS. MCUruntime139PASS/1ignored and strictallfeaturelibClippyPASS. Source topology5PASS. Original complete factory/Root preflight and no_std modules PASS.

## ARM evidence and limitations

Actual ARM ABI: current same-code owned kernel64216 B versus linked47192 B, removing17024 B (152 entries x112 B). This is type geometry, not complete memory fit. Linked-profile release ELF SHA2564f83f51d2e6b7b1b3041c65836e9fdb993451ca08f36a45f9e574d54d5ab518f links with49844 B flash margin; static209088 B (__sheap-__sdata), stack region61248 B. The static region and heap grant were not enlarged.

The firmware entrance still prepares/drops and refuses structured-fore-mask-unavailable. Linking this preparation entrance does NOT prove a complete application including reachable Play execution fits flash, heap or stack. No new physical sensing/display, admission, overnight history/restart or complete autonomous appliance is demonstrated. Devices remain on earlier firmware. DO NOT FLASH this as a qualified appliance.

## Latest local trace selection

The exact local-only factory now reserves zero remote lifecycle entries, with all237 ordinary Sign items and named quota intact. CPU originalfactory/preflight/no_std PASS retained159165/peak161204 (additional5688 saved, total51189 frombaseline). Shared local-Fore zero-capacity regression4PASS +strictClippy; actualLinesstillrequiretracecapacity. No new ARM release for this profile. The older134571 minimum below describes FULL remote trace stock, not the new profile; total fit remains unproved because metadata/workspace/software/baseline are additional. Resume from newest positive-local-trace receipt.

## Historical heap shortfall before local trace reduction

Read evidence/source76-ARM-heap-lower-bound-20261114.json: actual ARM kernel39592 + actual ARM Sign backing9006 + observed u8 canonical payload30449 + schema payload55524 =134571 B, exceeding the entire131072-byte heap by3499 B BEFORE slot/class metadata, software/workspace, original baseline and allocator overhead. This is a conservative selected-profile bound, not an actual runtime high-water mark. The CPU diagnostic copy observes stock only; original successful unmodified factory proof stays separate. Do not flash this profile or widen grants. Further real reduction is required.

## Fanout candidate, not delivered savings

Read source-fanout-opportunity-20261114.md/json:10 guarded local PreserveOrder schema groups,16 duplicate queue reservations,6616 B per-group payload estimate. No capacity changed. Lifetime/rollback/error/cancellation/rearm proof and class-wide packing effect remain unproved. Do not apply or count this estimate as a saving.

## Next substantive work

Read evidence/source76-execution-handoff-audit-20261114.md first: exact three-input/nine-output component, intentional prepare/drop-only service, existing admission/Start APIs, existing shared bus and carrier limits. Do not remove the refusal without installing the actual services.

First qualify the newly saved phase-byte integration on target, then continue substantial metadata/preparation-custody reduction under existing6341 and qualify actual target heap/stack including original Root, equipment, allocator and startup overlap. Mutable queues, phases, pending calls, refs and history stay funded in RAM. Do not widen grants or replace Source behavior with bespoke task loops to conceal refusal. Root command workspace and other immutable topology are candidates, not implemented savings.

Complete autonomous Source composition and actual admitted Fore/Face/Mask execution, durable history and restart/owner-loss recovery. Qualify the whole reachable executable and actual hardware before replacing working firmware. Preserve Body identity, observations and independent logical sampling/display. Seed fabrication must not hold this up.

Reproduce CPU proof with the selected generated OUT_DIR from the ordinary target build; tests require its exact emitted neutral_source_execution cfg witness. Internal target wrapper: scripts/run-rp2040-linked-terminal-build-20261114.py (adapt task/repo paths). Full receipts/logs under evidence; ARM sizeof command/source under scripts/arm-linked-terminal-geometry. Generated binaries remain local regenerable artifacts; hashes are recorded. Private keys, credentials and journals are deliberately excluded.

Current dev was refreshed and is already included. Threebee/owner UI is a separate coordinated lane (PR6895); use outputs/threebee-admission-handoff-20261111.md in the task workspace before touching its services or current owner context.
"""
 put('CONTINUE-HERE.md',note.encode())
 tree=git('write-tree');commit=git('commit-tree',tree,'-p',old,'-p',head,input=b'Preserve selected shared-stock Source factory and measured preparation savings\n')
 for p in paths:
  if (repo/p).is_file():assert run('show',f'{commit}:{p}')==(repo/p).read_bytes(),p
 git('update-ref','refs/heads/'+branch,commit,old)
 subprocess.run(['git','push','origin',branch],cwd=repo,check=True)
 receipt={'checkpoint':commit,'qualified_shared_parent':head,'source_files_byte_verified':len(paths),'private_state_excluded':True}
 (root/'outputs/origin-shared-stock-checkpoint-20261114.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt))
finally:
 if os.path.exists(index):os.unlink(index)
