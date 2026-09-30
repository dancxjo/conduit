# ConduitOS architecture proof appliances

This tree owns the bounded A-rung entrypoints used by repository proof commands.
They reuse ConduitOS machine and runtime mechanics, but they are not product
binaries and cannot be selected through ordinary product make.

The ordinary ConduitOS product entrypoints remain under `src/`. Architecture
proofs enter through `cargo xtask make conduitos ...`; direct Cargo binary invocation
is an internal build detail.
