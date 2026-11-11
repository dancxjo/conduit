# Pete cognition (development)

The authored Plot wires explicit typed Experience updates through retained
integration, evidence projection, bounded model context and `llm/generate-flow`.
Each committed revision produces one model request. Queue limits and the
model's input, context, output, work and history bounds are explicit. The result
retains the existing model-derived result contract.

Check the Plot and its supporting contracts through:

```sh
cargo xtask check pete-cognition
```

Current proof covers typed expansion, bounded integration and wording, installed
successive-revision processing, cancellation, and separate fake-provider model
execution in a shared Body Plan with a standing clock. The semantic codecs have
allocation guards; installed continuation tests verify stable kernel value-arena
capacity while their finite test collector owns output deliveries.

[The MVP](https://github.com/dancxjo/conduit/issues/3707) still needs the complete
Situation-to-Experience execution path, exact temporal context, selected
RAG/autobiographical feedback and actual outcomes, three cognitive cadences,
provider-loss isolation/recovery, a real-model two-episode demonstration and
Patchbay causal tracing. The shared Body model route currently executes calls
synchronously and reports provider loss as whole-Play failure.
