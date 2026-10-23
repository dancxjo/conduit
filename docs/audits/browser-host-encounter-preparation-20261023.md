# Browser Host encounter preparation (#6271)

Candidate 39915598198, reviewed head
`22348cc941d0978d066fec9d7b38839b6b7d890f`, tested merge preview
`1867b9d8ccbca774fe4c8d9dfbecabd073e12147`, passed every unit shard and
native architecture target. Its browser lane passed 267 of 269 Chromium tests.
The rendezvous and joined-Host-loss tests reached their unchanged 20-second
whole-test deadlines. Aggregate Candidate acceptance therefore failed.

Diagnostics artifact 12487542658 is 33,990,376 bytes, ZIP SHA256
`2c0961372c11420ebe2ff830129c68668a97cf9a1a9f5989269a0ec8ebab97ab`.
The original job log, both trace archives and error contexts remain retained.
The earlier full-inventory transfer repair remains useful, but did not eliminate
these distinct setup costs.

The rendezvous trace spends about 9.5 seconds preparing its real native Host
before navigation, then about 9.5 seconds on the ordinary default Browser Birth.
The deadline expires before Add a host. The loss test likewise spends about
9.4 seconds before navigation; its final full evidence transfer crosses the
deadline, with the four semantic assertions subsequently completing. These are
observer intervals, not calibrated product timing or input-to-paint measurements.

## Change

Prepare the installed native Host and the browser encounter concurrently. They
have no shared admission state before rendezvous. Await both before starting the
one-use rendezvous process and applying the original admission actions. Preserve
the same native Host, default workset in rendezvous, explicitly selected workset
in the loss test, exact UI routes and every semantic assertion. One Chromium
worker, zero retries, 20-second test deadline, 10-second Host readiness and the
30-second rendezvous connection budget remain unchanged. No additional Host,
browser worker, alternative product or synthetic membership is introduced.

The preparation helper waits for both branches. Browser refusal retires a Host
that becomes ready; dual refusal and cleanup refusal retain both causes. The
membership fixture also retires its process when readiness refuses. The loss
test selects just the four authoritative assertion values in the page instead
of transferring the full Workspace snapshot. No product code or authority
contract changes.

Five deterministic controls cover overlap before either preparation finishes,
Host refusal, late Host cleanup after browser refusal, cleanup refusal and dual
preparation refusal. The helper controls join the existing browser Node suite.

## Proof boundary

A focused pinned-Chromium diagnostic passes both affected tests with one worker,
zero retries and the ordinary deadline against a frozen copy of the previously
staged product and retained executable from the #6271 recovery checkout. This
is not a fresh exact-source product build. Raw local logs, artifact hashes and
the dedicated diagnostic inputs are retained in the task evidence. Fresh
combined-source Candidate, normal merge and relevant browser Integration remain
required before closing #6271. Original failed runs remain negative evidence.
