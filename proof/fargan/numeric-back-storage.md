# Retained numeric Back boundary

Central fixed-numeric and the affine/linear/embedding/compact tensor Flow
factories expose `prepare_with_inventory`, preserving concrete boxed root size
and local requested capacities before trait-object erasure. The existing Kernel
factory method delegates and returns the same prepared Back. There is no new
Kernel trait, altered Source guard, model arithmetic, or storage-limit increase.

`PreparedNumericBack` is opaque. Its private constructor receives the original
concrete prepared Back and its original local inventory. The receipt separates
root bytes and local capacities; checked summation refuses overflow via `None`.
This does not reserve preparation, include allocator bookkeeping, charge shared
model/tensor owners or their Arc headers, or charge the returned inline wrapper.
Existing local inventory may conservatively overcount evaluator storage.

AI library build and Clippy pass. The tracked standalone test now calls the actual
public FloatInteger factory from the immutable coherent AI SDK, retains the full
original archived Plan, and prepares its selected Back with unchanged guards:
local 6,392B + concrete root 2,672B = observed retained 9,064B. Receipt getters and
legacy projection request zero allocations; original prepared-output/cancel
behavior is retained. This test proves that actual public factory entrance and boxing boundary, not
execution of all other dispatchers or full model preparation/runtime.

Other direct numeric factory classes, shared owner deduplication, upfront full
preparation peaks, and aggregate public trained driver admission remain separate.
No #4898/#4907/#5218 completion claim follows from this component.
