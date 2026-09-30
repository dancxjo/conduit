# Effects and realization

> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

## Effects are inferred, not trusted annotations

Checking derives separate facts such as:

- external effects;
- temporal state;
- time/random dependence;
- resource dependence;
- suspension;
- determinism/variability;
- replay eligibility.

Do not collapse these into one `pure: bool`, and do not let an annotation lie a stronger property into existence.

Opaque/native backs must be confined to host-call/capability envelopes compatible with the semantic contract.

Provenance: #4070, #4071.

---

---

## Realization invariance

Enforce the constitutional substitution law:

> **Changing the selected Back may change performance, placement, resource use and evidence; it may not silently change authored semantic meaning outside explicitly admitted variability.**

Default Back eligibility requires the authored Kind's semantic contract **and** compatible checked Fore, not merely a same-shaped interface.

Therefore:

- same Fore + different Kind is not automatically substitutable;
- same Kind/Fore may have multiple eligible Backs;
- implementation-specific requirements belong to Back/offer/Plan truth;
- any permitted variability (approximation, stochasticity, fast-math profile, etc.) must be semantic contract truth rather than inferred from implementation names;
- planner diagnostics distinguish semantic mismatch from missing resource/availability.

This law applies equally to model providers, audio sinks, Mask stages, native/SIMD optimizations and other realization families.

Provenance: #4051, #3997, #4070.

---

---

## Replay, retry, movement, fusion and memoization

These are legal only when checked meaning proves the exact required law.

Effect-free deterministic bounded work may be eligible for recomputation/fusion.

Effectful work may be replayed only under a stronger explicit semantic law such as idempotency/transactional protection/known-not-committed effect.

Replan must not duplicate a committed external effect merely to reconstruct graph progress.

No blanket `retryable`, `memoize`, or `safe` trust annotation.

Provenance: #4072.

---
