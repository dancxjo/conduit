# Protected-service setup clearing observation

Clean Source `ac82545031d10044a9341f01066289e759cb0e13` completed
`cargo +stable xtask make conduitos ordinary-domain-proof --locked` on QEMU
10.2.1, q35/max, one vCPU and 256 MiB. The supported appliance accepted the
original 20-sample matrix and 14 additional setup-component records. All five
protection receipts bind the same Source, ELF and raw transcript. The index
retains those identities, exact source objects, machine arguments and hashes.

The additional interval brackets the existing service pool's complete
`View::clear`: page tables, image, exchange/output, stack, metadata, pixels,
scratch and trap storage. The clearing call, ordering and contents are retained.
Instrumentation and its retained counter field are enabled only by the proof
feature. Installation still uses the existing lease and quarantine lifecycle.

Across the 14 installations, clearing occupied 93.5304–96.6607% of the outer
setup interval, with median 96.5075%. These are raw TSC ratios from this one
appliance run. Both intervals include their counter overhead; root interrupt
time and cache effects have not been independently removed. The guest reports
`invariant_tsc: false`. The observations identify setup clearing as a measured
cost; they establish neither calibrated latency nor normal-product speedup.

The sidecar records carry the exact instance and outer setup value. The reader
requires all 14 records, in correspondence with the matrix's fresh and
reused-first installations, and rejects missing, duplicated, foreign-instance,
malformed or out-of-range complete records. Partial serial tails remain raw
evidence and cannot satisfy a declared record. Historical v1 transcripts that
declare no component records remain readable with an empty component list.
The baseline header's `component_attribution: false` still applies to the
undecomposed protected entry: the new sidecar attributes only slot clearing.

Three real fixed-buffer formatter tests and three reader tests pass. The actual
freestanding run also passes the existing ordinary computation, service memory,
compositor, hostile broker and copy-recovery checks. The five gzip receipts and
raw serial log are byte-preserving; the index lists raw and stored digests.
The ELF and ISO remain in the artifact directory identified by the index.

The earlier zeroing preflight examined the ordinary text/timer pool rather than
this measured service pool. Its partial-install warning does not apply here:
service installation claims a `Lease` before calling the fallible installer,
and failure drops that lease, clearing before releasing ownership. Successful
retirement and mutable-view invalidation of the cleared marker remain relevant
to any later optimization. This packet changes no clearing behavior.

Full #6183 acceptance still needs larger transfer-window/staged IPC sweeps,
independent trap/CR3/register/interrupt attribution and overhead controls, a
demonstrated optimization with paired protection equivalence, and a normal
responsive-journey comparison. Candidate, merge, Integration, stable publication
and physical results remain separate delivery gates. The original baseline and
its historical failures remain preserved alongside this extension.
