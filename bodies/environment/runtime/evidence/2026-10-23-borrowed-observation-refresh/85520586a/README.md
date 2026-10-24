# Borrowed observation proof refreshed onto current dev

Source85520586a is a clean normal merge of the previously verified test-only source874a611f9 with current devc40764ccf. Its product diff remains exactly two runtime test files. The entire environmental runtime tree is identical to the earlier874source; shared Plot/AI dependencies have advanced, so all owning runtime tests, supported current-dev preflight and strict all-target Clippy were actually rerun at855.

The previous actual negative controls and independent review remain at c6e08b231f413d16e9c8754220501d116c6f038c/bodies/environment/runtime/evidence/2026-10-23-borrowed-observation with their original execution identities. This packet does not relabel those historical executions. The refreshed tests exercise seven observation scenarios through owned borrowed-input tables after decoder payload/descriptors have been dropped, while retaining the original pulse-bind behavior.

This source is published and held outside the PR queue while omnibus6339 proceeds. No Candidate, accepted delivery, Integration, physical sensor or live Body Play is claimed here. Issue6313 remains open.
