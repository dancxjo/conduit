# Linked display packet schema

The actual selected electrical writer now borrows linked packet Type bytes instead
of retaining a decoded schema and another canonical prefix. Preparation compares
the complete checked contract Type. Execution retains full exact-envelope and
primitive validation plus the existing packet geometry/padding checks before any
pin effect. Command synthesis, formatting, Look policy, scheduling, capability
custody, failure and cancellation remain with their existing owners.

The CPU64 validator saves 794 retained heap bytes and becomes zero-sized. All 14
wire/selected-owner/refusal tests and 15 ordinary output-kernel tests pass, including
mandatory sign retention. A reference test compares exact error variants with the
previous owned validator for every truncation, a malformed primitive payload,
foreign prefix, trailing bytes and repeated valid use. Recurring execution has
zero Rust allocations. Strict package lint and wire-only dependency checks pass.

Logs are retained outside Cargo output at `outputs/environment-linked-display-packets`.
This is neither a complete owner-size nor an MCU memory measurement. No hardware
was flashed, no continuing Mask was installed, and combined sampling/display/network
admission and a complete recorder remain unfinished.
