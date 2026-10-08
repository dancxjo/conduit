# Actual trained storage-pressure continuation

The hosted lifecycle regression now executes a second two-epoch trained FARGAN
run through the public `PendingHostOutput<16384>` component. At the first actual
computed Host output it fills the existing 1024-slot store with separately owned
empty leases. The store refuses the output. Source cursors, scheduler decisions,
and published rows remain unchanged; the exact computed bytes and original
node/request remain retained. It releases only the injected leases, retries the
same output, completes the original pending call once, and continues the same
live scheduler. It neither invokes the owner again nor restarts/refeeds the run.

The complete Native outputs equal uninterrupted execution. Both runs drain two
epochs in 7003 services and 474 Host invocations, ending with no pending work or
stored values. Cancel, provider-loss and terminal pressure regressions still run.
The full gate passed in 428.05 seconds with 64 pinned inputs unchanged; Clippy
passed. No runtime/transport/store limit was raised.

Evidence is under the project outputs directory:
`outputs/fargan-model-compute-pressure-resume/{session-receipt.json,verification.json,test.log,frozen-inputs.json,clippy.log}`.
The 39,348,029-byte complete receipt SHA-256 is
`add154eaf9a815e5daf78db52b915945b82a508747a750818ef2e70b2888d593`.

This is an actual trained scheduler continuation proof in the existing hosted
fixture. It does not establish the forthcoming public concrete ModelCompute
driver, complete working/preparation inventory, target runtime, or full changing
200-epoch inference. The prior immutable model/Source/Plan and all earlier
failed/terminal session evidence remain preserved.
