# Next whole-workset preparation gate: Sign representation

With the qualified 32-byte resource-inventory bitmap, the original strict decoder accepts all 49 resource requirements. The signed workset is 30,314 bytes within the unchanged 32,548-byte input. Real factory preparation now reaches `Back("sign-storage")`.

The exact generated Sign policy has 237 items and 15,277 logical bytes. On this CPU ABI, `size_of::<KernelEvent>()` is 20 bytes, so `HostedSignLog::new_with_remote_storage` computes `physical_bytes = 237 * 20 = 4,740` and refuses `byte_capacity > physical_bytes` as `InvalidBudget`. Remote Sign extent is 5,688 bytes and is passed exactly to the constructor; it is not the observed invalid condition.

The planned byte ceiling comes unchanged from `fragment.sign_storage_budget.byte_capacity` through lowering into generated `SIGN_BYTES`. The concrete hosted Sign log uses bounded packed event entries, and publication charges a fixed `size_of::<KernelEvent>()` per event. Planned semantic Sign extents and concrete packed storage therefore need a deliberate general reconciliation. Do not clamp the Plan's budget to make this test pass or enlarge admission grants.

No kernel production edits were made by this child. The full kernel, shared workspace and complete retained preparation still have not been constructed. No admission, Start, physical IO or target heap-fit claim follows from this test.

Authoritative final diagnostic: `outputs/positive-sign-storage-diagnostic-20261114.log`, terminal expected failure after 21.25 seconds of the generated test. Earlier `positive-resource-bitmap-preparation-test-20261114.log` is preserved. Source/cache lease released.
