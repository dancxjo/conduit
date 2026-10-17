# Development failure records

These working-tree records document the development loop. They are exploratory
history, not stable acceptance evidence or clean-source receipts.

- `inference-offramp-red.log`: the proposed dataset-free public loader did not exist.
- `nonfinite-candidate-red.log`: a finite-loss/singular-gradient objective incorrectly committed a candidate.
- `batch-bound-red.log`: a signature could exceed the realization's batch admission.
- `evaluation-admission-red.log`: an invalid evaluation batch reached the objective before refusal.
- `checkpoint-exclusive-red.log`: shared-reference snapshot calls compiled despite the proposed exclusive staging contract.
- `rejected-cuda-1e-4/`: the first strict independent-training comparison failed; its manifest establishes nothing successful. The revised proof separates same-checkpoint inference from independent training and declares a fixture-specific `1e-3` bound before execution.

The fixes and verified examples are retained under
[the final tested source revision](../evidence/0d8aca837/README.md).
