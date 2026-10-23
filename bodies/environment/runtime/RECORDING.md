# Recording content

`cargo xtask prove environment recording` checks compact immutable recording
content and its ordered recovery. It generates the disposition Form from
`../profiles/recording.conduit` and uses the observation's existing authored
quantity Forms. Temperature and relative humidity remain semantic quantities.

A chunk owns at most 64 fixed-width samples and 4,096 JSON bytes. It retains
producer Host/Boot/Plan/Play and exact Type/Form identities. Failures occupy a
sample with their explicit disposition. Sealing allocates nothing and preserves
bytes; it does not publish them or claim durable completion.

The prepared reader validates complete canonical content before replacing its
loaded chunk. Ordered history recovery admits a finite chunk count, retains one
chunk, exposes sequence gaps across boundaries, and refuses overlap, replay,
reversed producer clocks, foreign context and capacity excess atomically.
Tests recover 64 chunks and 4,096 samples without execution allocations.

This is content preparation and CPU recovery. Root must independently admit
storage, exact generations, authority, membership and Lines. Publication through
the existing snapshot Host Call, actual browser persistence and overnight
physical operation are separate remaining appliance work.
