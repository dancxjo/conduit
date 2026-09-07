# Candidate evidence and reconciliation (historical)

Conduit previously exposed content-addressed candidate receipts, prospective
integration commits, explicit reconciliation, candidate retirement, and branch
retirement as repository operating procedures. That migration mechanism was
removed because it made contributors and agents operate CI internals by hand.

The durable lessons remain inside the implementation:

- validate the exact pull-request head;
- run only proofs appropriate to the current boundary;
- keep privileged deployment separate from untrusted code;
- preserve exact artifact identity; and
- fail closed when required release proof is missing.

None of those invariants requires a contributor to manage receipts, integration
SHAs, reconciliation labels, temporary refs, or workflow retirement.

For the current contributor interface, see [CI for contributors](contributing/ci.md).
For the development and release boundaries, see
[Integration and promotion](integration-and-promotion.md).

Git history retains the former mechanism if implementation archaeology is ever
needed. Do not restore it as a contributor workflow.
