# Native-owner handoff: bounded Application RTC remainder

Proposal only, not implementation. Audited source `7e9ccd0c6850d4f5782663f65ec7f5472109edb0`; reserved owner identity `ec37536e34754873b4e679e348feb4a097447f12` is a historical source/path inventory, not a statement that ownership has been released or that its code was tested here. No dirty owner content is included.

This follows the scope correction packet `a4274fcc640a417416476e5ed5471e131d988071`: full canonical Source policy migration is not a mandatory dependency of every bounded #6318 staging implementation; #6311/#6317 preemption obligations remain separate. Original issue criteria and historical source findings stay unchanged.

1. Checked Application-only native RTC remainder1..=64, enforced BEFORE actualentry.64 is theexistingDraft ceiling; no inventedgrant/profile, no SourceIRQconversion, no ambientoverride and no posthoc-onlyoverruncheck.

2. Immutable entry-local RTC consumption, Sourcecount separately, joined to actualserviceinstance/entrysequence/result/fault. Snapshot while nativeentryownership/interruptguard stillheld, before ACTIVEunlock; subsequententrymustnotreplace previousreceipt. No staleglobalreads or guestdeclaredconsumption.

3. Preserve existing Ordinary16/Draft64 entrypoints/callers andterminalfault/quarantine semantics. Add no activation, newoffer, scheduler/continuationauthority or full6317migration claim.

targets/conduitos/src/arch/x86_64/domain_transition.rs and its activecapture/continuation/assemblyfiles. CurrentTransitionReturn value/origin only; Budgetdrops thenACTIVEunlocks beforeServiceDomain globalcountread. No existingCostSnapshot at audited7e9/ec375. Nativeowner mustown receiptcarrier/interfacehandoff.

A caller-owned borrowed snapshot slot carried through EntryBudget might avoid textual transition edits, but changes that reserved interface's lifetime contract. It must receive explicit native-owner review; no unsafe pointer/global workaround or silent ownership bypass is authorized. The proposal stopped before branch/source/build work.

## Focused controls after handoff

- PureCPU testscheckedremainingreject0/65/MAX,accept1/64,Ordinary16/Draft64unchanged
- Receiptclearedbeforeentry,immutableRTC/sourcecounts capturedbeforeunlock, subsequententrycannotreplaceearlierreceipt
- Application-onlyprofile/instance/sequence/result correlation; wrongprofile/stale/reusedsequence/missingreceipt refuses and quarantines
- Loweredbudgethardwarearmenforcesremainingbeforeentry; actualnativefaultsterminal, no resumption/newcontinuationcode
- Privatecache/jobs2/debug0 strictcompile onlyif>=7GB; no fullnativeimage; retainactualELF/rawlogs andno runtimeproofclaim

## Eventual P0 acceptance

Stage sameApplicationCoreowner/protocol, oneoperation64ledger acrossallphases includingBeginWriteRead, finitezeroIRQprogress; measurecausalCommitphases andtotalfit64 withoutheap/timeout/retriesincrease. Originalnegative/refusal/atomicpublication/oneuseShow retained. ExactnormalPROFILEPatchbayaction+remainingStopreturnjourney/sourceELFISOHostBootACKserialQMPPNG, fullCandidate andmergedIntegration required. No6311/6317closure fromthishandoff.

No new Source grant, continuation/resume capability, production caller activation, P0 completion, physical display, CI acceptance or issue closure is claimed.
