# Cumulative audio frame projection proof

For #5674, extend the existing exact Source-owned projection rather than adding
an audio or speech numeric codec. The native thirds fixture now runs at 8000,
16000 and 48000 Hz. Independent integer arithmetic computes each cumulative
whole-frame count, per-step delta and retained remainder. Each three-step chain
ends at exactly one second's frame count with zero remainder.

The installed fixture checks the actual `audio/cumulative-basis-equal` and
`audio/cumulative-append` Source plots, expands them, selects their installed pure
expression implementation and runs their typed canonical inputs through a real
Plan/kernel Play and result Fore. Every projected delta/remainder is checked with
independent numeric assertions and compared to the admitted native receipt.
Source, Plan and Play identities and exact input/output bytes are printed as
focused evidence. Source time fractions, abstract timeline/origin and sample-rate
basis remain distinct from integer frame outputs.

The malformed installed input must refuse without partial output. A foreign
origin is refused by the existing Source-backed preparation owner before append. Source basis check and arithmetic are
separate operations: direct invocation of the arithmetic primitive does not
replace the complete owner's eligibility traversal. Existing native refusal
coverage checks foreign rates/anchors/denominators, overflow/profile bounds,
missing rate and forged cursor/result material.

This is local hosted conformance of generic audio operations. It claims neither
an active hardware sample clock nor PCM replay, browser execution, acoustic timing
or the full conversion/replay acceptance of #5675. The eleven native tests and two installed tests pass. Eighteen successful
installed Source/Plan/Play rows retain exact canonical input/output evidence.
Strict Clippy with warnings denied passes for both focused test targets.
