# Parallel preparation in existing browser Host proofs

Owning issue #6271; integration handoff #6268. This changes only two proof specs,
not the product, default Plots, canonical Host inventory, action contracts or
20-second test deadline. One pinned Chromium worker and zero retries remain.

Candidate 39915598198 on 22348cc941d0978d066fec9d7b38839b6b7d890f has 267 passing
browser cases and two global-budget failures. Rendezvous navigation starts at
+9.658s after Host preparation; Birth finishes at +19.254s, membership opens
after the deadline, and Add a host is attempted at +20.404s. The error DOM
contains that button. This fails before the earlier compact-evidence repair,
and is not evidence that the control is missing.

The joined-Host loss case starts navigation at +9.472s. Admission, offer count
and loss status pass; full evidence transfer starts at +19.549s and returns at
+20.484s. All final loss assertions pass at +20.516s, after the global deadline.
The snapshot shows the remote Host offline and local browser Host present.
These are failed tests despite those later assertions; the negative logs and
DOM contexts remain [retained](evidence/browser-parallel-preparation-20261023/).
Full diagnostic traces remain in GitHub artifact 12487542658 of that run.

Real Host-service preparation and actual browser Birth/setup now overlap inside
the same deadline. Both must complete before rendezvous starts and before the
invitation/admission flow. All real actions and semantic assertions remain.
The loss case transfers only the Body identity and exact membership/offer
counts used by its assertions, leaving the complete inventory authoritative in
the page. Failed preparation retains its original error while cleanup awaits
both owned processes and bounds termination. No work moves outside the test
budget, and no timeout, retry, forced interaction or polling action is added.

The first focused diagnostic passes both cases in 15.1s. A subsequent cleanup
refinement and removal of an unnecessary environment override were followed by
one final verification of committed specs 6f0bfbe8f: **two cases pass in 17.0s**,
one Chromium worker, zero retries, unchanged 20-second deadlines. Node syntax
and whitespace checks pass. No source changed during that final invocation.
The existing staged Workspace product and CLI were reused: these runs establish
focused proof-code behavior, not fresh exact-source product acceptance. The
integration owner retains the fresh Candidate gate; no duplicate PR or full
269-case local/CI campaign was started.
