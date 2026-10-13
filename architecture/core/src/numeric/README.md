# Exact numeric denotations

`MathValue` has an inline `i64` case, a reduced exact rational, or a finite symbolic DAG. It does not choose an approximate realization. Integer/rational arithmetic is exact or returns a resource/domain refusal. Unsupported symbolic equality returns `Unknown`; representation identity hashes canonical wire bytes.

Integer magnitudes use at most 128 base-1,000,000,000 limbs (1152 decimal digits); temporary arithmetic has 256 limbs. Euclidean reduction permits at most 8192 iterations. Symbolic DAGs have at most 128 nodes and depth 32. Canonical wire is capped at 32768 bytes. These are admission/work bounds, not a promise to represent all real numbers or decide all identities.

Wire begins with version 1 and a tag: 1 for an inline signed little-endian `i64`, 2 for a sign and reduced numerator/positive denominator (minimal little-endian base-1e9 limbs, each preceded by `u16` length), or 3 for a `u16` node count and topological DAG. Symbolic literal nodes contain a length-delimited non-symbolic value; operation nodes contain an opcode, root degree if applicable, and prior operand indices. Shared nodes occur once, all nodes are reachable, and commutative children are ordered by exact node identity. Node order is canonical depth-first postorder. Version, normalization, arity, cycles, redundant nodes, domain guards and trailing bytes are checked before allocation.

`MathValueRef::validate`, intrinsic profile admission, identity hashing and copying an admitted borrowed wire value use no heap allocation. Owned construction, arithmetic, symbolic preparation, `to_owned`, and `MathValue::encode_into` may allocate within the bounds. The latter encodes an owned value through a preparation buffer; `MathValueRef::encode_into` copies existing admitted bytes without allocation.

`PortableNumericProfile` binds admitted forms, limb/node/depth limits and wire storage to its intrinsic `numeric/profile-...@1` kind identity. It does not certify a numerical error. Smaller profiles can transport finite Data values without reserving the maximal symbolic budget for every sample.

Trig methods explicitly take or return turn coordinates. Angle typing and dimension/family checks belong to the checked computation/physical layer. Pi is nonzero and positive; cancellation requires a proven nonzero denominator. Quarter turns and rational Pi factors simplify exactly. Other supported trig/root expressions retain their denotation; unknown domain proof returns a refusal.
