# Print lifecycle inspection in Observatory

Development work for #5737, under #5234.

`inspect_print_sign` accepts the existing retained `SignArtifact` envelope. It
uses the existing exact Sign/Plan/Play validator, finds the producing placement
inside the matching Host/Boot fragment, requires an admitted typed Print
lifecycle output, bounds its encoded size, and decodes the Source-owned
`PrintLifecycle`. Its inspection borrows the retained evidence and provider.
It does not construct another job state machine or infer an event from a
placement's generic terminal status.

The text view distinguishes queued, dispatched without observed ACK,
provider-acknowledged, completed, cancelled before dispatch, provider-reported
failure and uncertain delivery. Unknown ACK, provider loss after dispatch and
cancellation after dispatch retain their exact typed reason. Uncertainty never
grants permission to retry. Exact job, attempt, provider job when known, Source,
checked/expanded Plot, Plan, producing provider/implementation/artifact,
Host/Boot, placement and Sign remain inspectable.

This is a read-only projection of supplied typed provider evidence. It does not
send jobs, implement a printer protocol, authenticate external equipment, or
make a fixture into physical delivery proof. Missing exact context, a different
provider, a non-provider event, a wrong output Type, malformed bytes and an
oversize envelope are refused. Parent #5234 retains broader acceptance.

The ordinary `build_report` / `render_text_report` path also projects current
and explicitly historical typed provider events. Raw Signs remain retained.
No generic Plan completion is converted into printer completion.

Local validation: 56 tests (52 existing Observatory tests and four Print tests),
strict all-target Observatory Clippy, and pinned workspace formatting pass.
These are hosted deterministic projection and refusal tests. No issue closure
or physical printer proof is claimed here.

Fresh validation at `753c4f97031664616e9f1a7331c26acdbd9af681`, after accepted development
`ac97b99fd9016e5879f9822432fe945ee3c500f0`, passes all 56 tests, strict all-target
Observatory Clippy, thumbv6m compilation and pinned workspace formatting. Source
inputs were refreshed unchanged to exclude relocated-worktree Cargo reuse.
The implementation and narrow projection proof class remain unchanged.
