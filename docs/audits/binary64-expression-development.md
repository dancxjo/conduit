# Binary64 expression computation (development)

The contextual expression literal `0xXXXXXXXXXXXXXXXX` in an `F64` result or
operand position preserves exactly sixteen hexadecimal digits as IEEE binary64
bits. Integer contexts keep their integer meaning. Decimal-to-float conversion
is not implicit. Raw equality uses Conduit's established encoded-bit identity:
the two signed zeros differ, and NaN payloads retain their identities.

Checked expression addition, subtraction, multiplication, division, remainder
and ordering now support binary64 alongside the existing binary32 profile.
Each arithmetic operation rounds at its selected machine width, in the authored
operation order. These are **finite checked operations**: nonfinite operands,
nonfinite results and zero divisors refuse. Ordering refuses nonfinite values;
it compares finite numeric values rather than encoded-bit order. This profile
does not implement nonfinite propagation, selectable rounding directions or
IEEE exception flags. Those original #5392 acceptance requirements remain open.

The reference and prepared evaluators share the finite arithmetic policy. The
prepared evaluator also preserves raw binary64 equality, which previously
fell through to unsupported integer comparison. The scalar tape continues to
refuse float programs; the std source offer preparation can select the existing
prepared reference realization instead, retaining the tape refusal in Plan
selection evidence.

`architecture/plot/tests/ieee_f64_expression.rs` checks all pairs of signed zero,
subnormal, infinity and distinct quiet/signaling NaN bit specimens for equality;
contextual literal identity; finite operations and refusals; finite ordering;
and recovery after a refused evaluation. The cancellation cases distinguish
binary64 from binary32 intermediate rounding and exact final-only rounding.
Repeated prepared operations retain their output capacity with zero observed
allocations or reallocations. The actual CLI product corpus separately exercises
the binary64 cancellation through ordinary Source, planning, Play and report
verification. These are development proof surfaces, not stable-main acceptance
or a certificate for arbitrary floating-point algorithms.
