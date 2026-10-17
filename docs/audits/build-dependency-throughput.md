# Source generation through optimized build dependencies

Development and test builds run ordinary Source checking and Rust binding
preparation inside build scripts. The observed Human generator spends CPU on
checked keymap and native Type preparation; test optimization alone does not
optimize the separate build-dependency graph.

The workspace now sets `opt-level = 1` in both development and test
`build-override` profiles. It changes no authored Source, numerical law,
checker, binding generator, runtime profile or timeout. Cargo artifact metadata
confirms assertions and overflow checks remain enabled for the generator in
both build entrances.

Three direct generator trials per variant retain identical generated Rust bytes
and identical stdout/stderr hashes. The baseline is source `bcef0e4ade69ce43ab838871c8b94a395169fc1a`;
the optimized measurement source is `4c6d53e8f074c1dd5a389de944a6ab6d1234c4f0`. Those trees differ
only in `Cargo.toml`. Each executable was produced by Cargo, then measured with
an isolated output directory against the original generated-file hashes.

| Generator profile | Median wall seconds | Median user + system CPU seconds |
| --- | ---: | ---: |
| Default unoptimized build dependencies | 124.007 | 123.997 |
| Build dependencies at optimization level 1 | 7.004 | 7.004 |

[Raw trials, artifact hashes, flags and generated-file digests](evidence/build-dependency-throughput.json)
retain every sample. Wall time uses a monotonic clock; CPU uses child-process
user and system time. Host frequency, affinity and concurrent development load
were uncontrolled. This is one native preparation workload, not a calibrated
cross-machine benchmark, Conduit execution speedup or whole-CI ratio.

Optimizing dependencies adds compilation work and distinct cache entries, so
trivial build scripts may not benefit. The observed Cargo builds took 117 and
67 seconds with different warm cache states; those totals are not a controlled
clean-build comparison. Source-pinned integration must establish the overall
pipeline result. No timing threshold or retry is introduced.

Validation passed all eight Human library tests, strict library Clippy,
formatting and patch checks. The test entrance separately confirms the
optimized generator flags without rerunning the completed test corpus.
