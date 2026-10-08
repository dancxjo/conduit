# Separately bounded U16 value-contract-only preparation

The canonical `PreparedU16Profile` domain remains unchanged, including authored
invariants. A separately named contract-only construction capability refuses
invariant-bearing profiles before allocation. It supports the original FARGAN
`type FarganPeriod = U16 in 32..=255` declaration and full-U16 profiles without
invariants, retaining every original value contract and unchanged Step checks.
General invariant evaluator preparation remains a separate unsupported profile;
no law is dropped, approximated, or claimed checked by this limited entrance.

Allocation-free preflight counts both owned Type clones, the two-byte scalar
allocation, both existing Core Value::finish encoding temporaries, exact final
canonical framing, full Core validator reservation, exact contract array and
complete contract clone reservations. For existing Core finish, the Type-prefix
Vec grows once to twice its capacity to append the seven-byte U16 leaf node;
both original and replacement requests are charged. The final canonical writer
uses Core's exact-size bounded writer, retaining identical canonical bytes.
The private zero output template remains original construction machinery, never
admission of zero into a restricted Source domain.

The selected factory verifies the exact original retained offer allocation-free
and adds the concrete Box root to the upfront request/retained reservations.
Profile/offer/map acquisition/preparation, allocator bookkeeping and stack remain
separate. This is not full archived factory or trained runtime proof.

Two tests pass, plus AI library/test Clippy. Original FARGAN period construction
requests984B, peaks/retains481B within1152B/649B conservative source-capacity
bounds. Full-U16 construction requests928B, peaks/retains457B within1096B/625B.
All four one-under cases and a valid invariant-bearing Source profile refuse
with zero allocations. Runtime domain guards and legacy entrances are unchanged.
No whole driver, general evaluator preparation, or #5218 acceptance is claimed.
