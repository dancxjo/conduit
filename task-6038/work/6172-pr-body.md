The two-batch Chromium experiment failed [combined Integration](https://github.com/dancxjo/conduit/actions/runs/39500212975): four slow-batch failures and 29 unexecuted cases, while the other batch passed. Restore the single-process, one-worker reference as the default. Keep the experiment explicitly opt-in for controlled diagnosis and update its recorded status. Refs #6172; blocks admission of #6165 until combined development is healthy.

The checked spec inventory, sealed source product, Chromium project, zero retries and evidence validators remain intact. Logs/traces establish unfinished preparation/timeouts; resource contention remains an inference.

Twenty focused pipeline/runtime-selection checks passed. Actual Chromium Candidate and exact-head combined Integration remain required. No failed workflow was rerun or reclassified as acceptance.
