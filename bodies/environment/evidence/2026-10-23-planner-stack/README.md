# Optional numerical verification stack

Issue [#6321](https://github.com/dancxjo/conduit/issues/6321) and isolated current-dev
[draft PR #6322](https://github.com/dancxjo/conduit/pull/6322) own the two-file
planner delta. The appliance checkout carries the same implementation.

All 84 planner library tests, strict library Clippy and the supported
`cargo xtask check angle-reference` proof passed. Current-dev preflight passed
workflow/lock checks and 345 CI-tool tests after restoring the sparse checkout's
required fixtures. The ESP32 release build passed.

Internal diagnostic disassembly (`xtensa-esp32-elf-objdump -Cd`) measures the
ordinary candidate-check frame at 64 bytes, compared with 18,144 in the prior
image. Exact numeric candidate verification retains its 18,144-byte frame behind
a non-inlined optional boundary; request decoding retains 10,400 bytes. Both are
entered only for numerical realization requirements. The ordinary path still
validates realization properties; numerical work bounds, certificate admission,
refusal precedence and ranking retain their existing behavior.

This is function-frame evidence, not complete call-graph stack admission or
physical worker/start success. The physical preparation workflow is recorded
separately when it terminates.
