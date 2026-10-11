# CDC terminal drain native evidence

The checked Source at `08cbf7798efd6fba531e8a5812a9117f3cfb7d4f` runs on both existing product kernels at `1f5a7297820d1de826b7d4e9bec6c2d80baacebf`. The Source package changed; the product kernels did not. Each receipt binds these separate identities, exact media, transcripts and validation tools.

Eight cases pass: normal receive, binary short receive `ff0080`, six-byte line-coding response, and wrong 9600-baud line coding, on x86_64 and AArch64. Supported cases retire 32 ordered Serial results (16 transmit, 16 receive). Both coding refusals retire exactly one typed unsupported result, with four Source controls, two discovery controls and no control-state or bulk effects after the refusal. The Source drains already queued commands without effects, then closes; its seeded-state terminal flag becomes true only on closing.

Nine focused checked-Source/kernel tests passed with this Source, including one and 32 queued commands and output pressure. The previous native coding-refusal failures remain unchanged in the sibling `coding-refusal-1f5a72978` records. These new observations resolve that demonstrated defect; they do not prove all cancellation, unplug, stall or late-completion paths.

Proof is actual freestanding product controller traffic through an owned external USBredir device under QEMU. Physical compatibility, full substrate and serial acceptance, other USB classes and final cross-architecture acceptance remain unverified. Tool source snapshots supplement original per-run validation hashes; kernels and media remain locally retained and are bound by receipt hashes.
