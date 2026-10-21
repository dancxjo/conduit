# Failed Application preparation checkpoint

This branch is a recovery checkpoint, not an admissible PR update. Its staged initialization and shared codec workspace do not pass normal-workset admission in the unchanged private heap. No further build, VM or allocator-placement attempt is authorized by this checkpoint.

The preceding PR source06eb3a884 builds, but the complete ordinary journey refuses Plan. One diagnostic atffd0479fe proves Application Commit WorkExhausted at64RTC interrupts. That image is d3a9240a864c7714d0d23d643414a86e295e9a374f7568f2d346410139bd07e4; diagnostic serial SHA256461fcc8304c9d93318f1ed93d23746504835f996a27f3158ee9da8f6ff208135. Full raw journeys and sealed images are retained in the task workspace; this small checkpoint does not duplicate those media.

The subsequent candidate splits typed initialization across at most22commands, preserving hidden preparation and publication after final reply encoding. It reuses actual JSON, compressor and reply storage admitted before controller construction. Codec and graph digest bytes remain unchanged.

Sixteen unit/byte-identity cases pass. The attached exact production allocator test FAILS during staged initialization: a131072-byte allocation is refused at1811168rounded live bytes, peak1843936, within the2072576-byte heap. Total free bytes remain261408; fragmentation prevents the allocation. This log was captured from the dirty candidate based onffd0479fe before the recovery commit; it is not clean-source acceptance or native proof.

No region grant, RTC budget, codec format or allocator policy was increased or changed to force a pass. The next implementation needs a concrete typed-allocation/admission design; further padding or placement trials were stopped. #6204/#6205 remain drafts and #6207 remains blocked.
