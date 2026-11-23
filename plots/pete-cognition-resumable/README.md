# Resumable Pete cognition

The same cognition Gears use an episode State Back whose exact initializer is
selected before planning. The hosted application loads that initializer from
the Body and Wake checkpoint journal. The existing scheduler emits the initial
memory and advances it on each completed episode. The checkpoint Fore lets
the host persist each committed state using the synchronized file residence.

Each frame carries battery charge and charging state. The Situation Gear uses
the reviewed homeostasis policy to add derived energy and safety context while
preserving the raw observations.

The checkpoint holds the completed episode count and at most four selected
observations and actual attempt outcomes, within the same 8 KiB bound as resident memory. Oldest entries are evicted as needed
to fit. Recovery checks the owner and count before any recalled history can enter
a model request. A replacement Play continues the episode count in the same Wake.
Recovery upgrades the previous successful-only checkpoint profiles. Selected
observations made during provider loss retain their failure metadata and can
be recalled after restart.

Run `cargo xtask check pete-cognition` for the proof. It commits the first Play's
checkpoint, reopens the journal, then selects the recovered bytes in a new
Host Boot and Plan. A fenced model verifies that the next episode recalls
the retained switch label. The application supplies the observation frames and
handles the checkpoint Fore. This proof covers graceful replacement and
synchronized checkpoint recovery within one Wake.

The same entrance includes the finite checkpoint/trace turnover conformance
from [#7029](https://github.com/dancxjo/conduit/issues/7029). One ordinary
resumable Play receives 32 full synthetic frames and makes 32 fenced model
calls. All 33 checkpoints preserve their owner and ordered completion count;
surviving memories remain complete. Each byte eviction is checked against the
nearest excluded suffix, encoded only in the test after Play. The first run of
this corrected contract retained episodes 30–32 in 8154 bytes; adding episode 29
required 10257 bytes, exceeding the unchanged 8192-byte limit. Maximum model
request size was 3791/4096 bytes. The earlier exactly-four occupancy expectation
was invalid, and its failed observations remain separate.

The actual debug history retained 128 records and reported 692 evictions. Replay
preserved the gap and unavailable parent ancestry with
`exact_reconstruction=false`. This is deterministic finite turnover proof with
a fenced model, not a timed soak, live model quality, physical sensing or
stable-release acceptance. [#6878](https://github.com/dancxjo/conduit/issues/6878)
still owns sustained load, slowdown, cancellation, recovery and independent
cadence proof. Raw local records remain under ignored `target/pete-trace-horizon/`;
durable publication is separate.

The same deterministic entrance also checks the clock boundary from
[#7036](https://github.com/dancxjo/conduit/issues/7036). Each case uses two
native Host instances with distinct Host/Boot identities and the same checkpoint
owner. A shared declared clock basis recalls the original source timestamp,
nonzero uncertainty and source Sign. With a different basis, even the same
numeric counter produces explicit `clock_mismatch` omissions and scoped
`no_history`; the archived evidence remains byte-identical. The first runtime
pair passed with maximum model request 3398/4096 bytes and checkpoint 3327/8192
bytes, making one fenced model call per Play.

These clocks are explicitly injected test evidence. This proves neither actual
clock correlation nor relation-backed transfer. The public operator's clock
producer and the missing relation ingress remain separate prerequisites under
[#6879](https://github.com/dancxjo/conduit/issues/6879). Fixture Host/Boot names
identify each case's configured realizations, not globally unique real machines.
