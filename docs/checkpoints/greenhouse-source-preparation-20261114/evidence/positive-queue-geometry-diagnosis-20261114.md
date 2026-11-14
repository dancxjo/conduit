# Next actual factory gate: queue geometry differs from Cord count

The complete Source76 factory's independently linked terminal laws match every actual prepared Back report: all76 nodes and both ports, no mismatches. This was observed read-only through the CPU-only factory copy; original checks and original `PreparedFragment` invocation remain intact.

Original factory SHA-256: `3251c2ad4a4511e1aa34e90f46feb38ef398e4ec5ba7992ae706953f3377c4fe`.
Diagnostic copy SHA-256: `45aa6b5e43680731e06080ac10093f4735704c3e783a26fc9b625e9339c09cdc`.

The scheduler still returns `Storage(Topology(InvalidPlan))`. Exact immutable projection decoding explains the failure:

| Cord | Queue start | Item capacity | End |
|---|---:|---:|---:|
|7|7|2|9|
|39|40|2|42|
|93|95|1|96|
|94|96|1|97|

There are95 actual Cords and97 actual queue slots. All other Cords have item_capacity1. `execution::ExecutionKernel` currently aliases `FixedScheduler` with `QUEUE_SLOTS=C`, and the neutral factory usesC=95. `validate_plan` correctly refuses an end beyondQUEUE_SLOTS for Cords93 and94. The two two-item feedback queues are already in the checked Source/Plan and cannot be erased to make preparation pass.

The same audit finds92 route targets with no duplicate/missing local-source membership, all local ports below2, no zero item/byte capacities and no maximum-value quota beyond each Cord's byte capacity. This is a geometry assembly mismatch, not a mismatching terminal law. Correct derivation must retain original semantic queue capacities and separately fund their actual physical slots; no execution authority or resource ceiling follows from deriving that count.

Authoritative terminal report log: `outputs/positive-terminal-report-diagnostic-20261114.log`, generated test terminal failure after16.61 seconds. All Kernel layout production work is qualified and committed as4a656534c1. No queue/runtime/Source edits were made by this child in this diagnosis. No admission, Start, physical IO or target fit has been proved. Compiler/cache leases released.
