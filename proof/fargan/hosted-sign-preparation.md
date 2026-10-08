# Hosted sign-array preparation admission

`HostedSignLog::{storage_reservation,new_with_storage_limits,owned_heap_bytes}`
charges complete local and remote Option slot arrays separately from logical
Sign byte limits. Existing constructors and record/remote behavior are unchanged.
No capacity was raised. Roots, allocator bookkeeping and stack remain separate.

Three focused tests pass: exact requested/live capacities and both one-under
zero-allocation refusals; local/remote runtime parity with zero allocation; and
108 legacy budget combinations preserving exact refusal behavior. Local1024
slots request/retain20,480 bytes; local32+remote4 request/retain736 bytes. Kernel
alloc library and test Clippy `-D warnings` pass. Evidence is in the goal project's
outputs/fargan-hosted-sign-preparation. This does not establish whole runtime
working/preparation admission or any speech acceptance.
