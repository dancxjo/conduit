# Failed Application preparation checkpoint

This branch is a recovery checkpoint. Its staged initialization and shared codec workspace fail normal-workset admission in the unchanged private heap. Allocation experiments stopped after this failure; no subsequent build or VM attempt was made.

PR source `06eb3a884` builds, but the complete ordinary journey refuses Plan. One diagnostic at `ffd0479fe` proves Application Commit `WorkExhausted` at 64 RTC interrupts. Its image SHA-256 is `d3a9240a864c7714d0d23d643414a86e295e9a374f7568f2d346410139bd07e4`; serial SHA-256 is `461fcc8304c9d93318f1ed93d23746504835f996a27f3158ee9da8f6ff208135`. Full raw journeys and sealed images remain in the task workspace.

The subsequent candidate splits typed initialization across at most 22 commands. Preparation stays private until the final reply is encoded. It reuses JSON, compressor and reply storage admitted before controller construction. Codec and graph digest bytes remain unchanged.

Sixteen unit and byte-identity cases pass. The attached production allocator test **fails** during staged initialization: a 131,072-byte allocation is refused at 1,811,168 rounded live bytes, with a peak of 1,843,936, in the 2,072,576-byte heap. Total free space is 261,408 bytes; fragmentation prevents the allocation. The log came from the dirty candidate based on `ffd0479fe`, before this recovery commit. It is not clean-source acceptance or native proof.

Region grants, RTC budgets, codec format and allocator policy are unchanged. The next implementation needs a concrete design for typed allocation and admission. Further padding and placement trials were stopped. #6204 and #6205 remain drafts; #6207 remains blocked.
