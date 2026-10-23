# Separate structured equality preparation frame

Revision `2dc324d05` extracts structured equality preparation into a non-inlined
helper. Primitive Binary nodes retain the existing primitive branch and no
longer need the structured evaluator's local preparation frame. No equality
semantics, admitted bounds or validation conditions change.

Six structured equality tests and three scratch-storage tests pass. All 19
32-bit environmental publication/kernel tests pass, including the seven
960-sample workloads. Plot library Clippy passes with warnings denied.

These receipts prove CPU behavior. The Xtensa firmware rebuild remains active
at the time this receipt was written; stack-frame reduction and physical startup
have not yet been proved for this extraction. The device was restored to the
known-good fixed-memory network image after the preceding failed experiment.
