# Frozen checkpoint preparation result

The CPU proof uses Blue's privately retained real committed source and independently pinned original Body and Root. Canonical membership completes, persists and reopens a verified new credential without replacing the real custody prefix. The 10,368-byte output takes 81 chunk preparations and 83 complete recipe invocations in the current replay writer.

A bounded recipe builder now shares the canonical frame encoder: changed prefix, optional append and suffix bytes are retained in explicitly reserved buffers; unchanged archive bytes remain a source range. Ordinary encoding and the captured/reassembled recipe have equal digests. Actual joined output contains 8,181 changed encoded bytes and 2,187 reused bytes. Used lengths are not retained capacities or target memory grants.

Before enabling direct recipe chunks, the writer now binds the programmed payload to its prepared digest before commit, reusing its existing readback hasher. The staging-corruption regression fails before this change and passes after it. All nine incremental checkpoint tests, including 57 physical cut injections, pass. These are CPU media tests, not physical flash interruptions.

The attached summary distinguishes the exact CPU proof source, matching seam-file hashes and exact offline ESP32 build source. The current candidate fits the existing app partition and unchanged stack floor. It was not flashed. The frozen recipe is not connected to the live writer; source-block tamper checks, exact workspace reservation and target phase timing remain required before admission can be enabled. The reviewed preparation policy remains absent, so live membership remains deferred.
