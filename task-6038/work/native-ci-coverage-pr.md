The full workspace run that found #6140's native chunk fixture failures also exposed a CI coverage gap: delegated ConduitOS commands used default features, excluding the owner-network tests entirely. A green default-feature job could not exercise those regressions.

Run delegated ConduitOS library, integration and documentation proof with the workspace's explicit `hosted-tools,native-compositor` profile. Include the three gated compositor integration targets, retain the existing disjoint test partitions, and reject unclassified or missing required targets. Check delegated features against Cargo's actual workspace feature inventory. Narrow admission preserves the same native feature context for tests and Clippy while retaining current package-scoped lint and preflight-only behavior.

Follow-up to #6140; an integration prerequisite for #5994 and #6038. No product behavior, default Cargo features, execution limits or timeout caps change.

Validation: 28 admission Node tests pass on this refreshed source. The two Rust coverage files are byte-identical to the prior local source that passed 13 focused tooling tests and strict tooling Clippy. Current-source tooling validation and the three newly selected native integration targets are running; their results will be recorded before readiness. This is not a full workspace, Candidate or stable-acceptance claim.
