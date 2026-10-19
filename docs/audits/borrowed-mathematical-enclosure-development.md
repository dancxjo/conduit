# Bounded consumption of borrowed mathematical data

Private Core preparation for #5392 adds a prepared consumer of the existing
canonical mathematical-value encoding. A caller can construct π/180 or principal
sqrt(2) after preparation, then obtain an exact rational interval without growing
the evaluator's storage. The owning enclosure and borrowed consumer use the same
per-node arithmetic, constant series and root refinement implementation.

Preparation reserves the existing 32-node profile and admits source extent,
scratch payload, retained payload, limb work, interval width, constant terms and
root refinements. Execution validates the borrowed canonical source and applies
the smaller of the caller's remaining work and the prepared work ceiling.
Consumed arithmetic work is charged on refusal too. Storage figures count vector
capacity and the container; allocator overhead, stack frames, caller input and
returned result storage are excluded. They are not whole-process bounds or WCET.

A successful result borrows only the caller's source bytes and owns its endpoints.
Later calls cannot change an earlier result. Failed partial scratch remains private
and is cleared before the next evaluation. No failure emits a previous success.
Unsupported forward trigonometric enclosure, insufficient series/refinement,
malformed input, exhausted work and insufficient tolerance remain named refusals.

An independent verification entrance recomputes incoming bounds and width. An
optional separately retained source pins byte representation custody, without
claiming mathematical equality across different encodings or producer execution.
It rejects changed source or claims and returns only recomputed metadata. The
existing generic projection framework retains the same interval law, precision
loss facts and policy identity. Borrowed ownership does not create a second
fidelity protocol; false exactness, missing facts, wrong targets and changed
policies refuse.

Seven focused tests exercise 32 repeated joined construction/consumption cycles,
owning/borrowed endpoint parity and independent established decimal brackets for
π/180 and sqrt(2). Allocation guards cover successful evaluation, claim
verification, projection report admission/refusal, failure after partial work
and valid recovery. No allocations or reallocations are observed in those guarded
calls. Admission tests check exact scratch/retained boundaries and finite
requirements. These are component tests, with no installed target or speed claim.
The local Linux x86_64 capture records one preparation allocation, 65,792 bytes
of interval scratch and 67,920 bytes of retained payload. This fixed reservation
is paid even for small inputs. All 144 selected Core regression tests, strict
Core all-target Clippy, Core without default features and formatting pass.

This work does not add symbolic Quantity wire, Source syntax, physical Type
binding, Plan/Play/Sign integration, producer attestation or an installed Back.
It does not establish #5392 acceptance. Those ordinary-pipeline boundaries and
the original issue's broader requirements remain open. The existing prepared
binary64 algorithm separately retains actual intermediate rounding; normalized
exact mathematical data cannot replace that computation witness.
