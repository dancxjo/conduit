# Canonical Source cache observations for #6180

The existing #6087 Source cache now offers `check_observed` and `expand_observed`.
The original `check`/`expand` entrances delegate through no-op observers and
retain their signatures, dependencies, identities, retention limits, hit policy,
eviction and refusal behavior. The observer receives only finite Rust events;
its return value cannot select semantic outcomes or establish authority.

Cold checking reports actual parse start/end and check start/end with the
checker's acceptance boolean. A check hit reports only CheckHit. Cold expansion
reports its actual start/end and acceptance; a hit reports only ExpansionHit.
No timing clock, native platform dependency, event collection, Host state,
Plan/Play/Sign, diagnostic wire or extra cache is added to the canonical layer.
Event delivery itself allocates no collection. Callers own their observations.

This preserves an important dependency distinction: changing a Kind revision
can invalidate expansion while the checking signature remains reusable. The
observer reports that actual check hit and cold expansion. Removing a signature
that checking read causes real checking and its original refusal. Failed checks
and expansions remain uncached; zero-retention mode remains the ordinary cold
path. Authoring expansion still supports open Fores; executable-root closure
remains a separate required admission law.

18 cache integration tests pass, including five new cold/hit, exact owner/cold
meaning, refusal/non-retention, dependency distinction and zero-retention checks.
All 472 owning-package unit tests pass; one existing manual observation remains
ignored. Formatting and diff checks pass. [Retained checks](evidence/canonical-preparation-observations-20261020/).
An initial new test incorrectly assumed that an expansion-only revision changes
checking; the existing dependency-aware cache correctly hit. The corrected
checks exercise actual checking dependencies and preserve that behavior.

## Scope and integration dependency

Draft #6191 actively owns native workset caching, startup and journey paths.
This change is disjoint: only the canonical cache, its narrow export and tests
are changed. No draft code, runtime-budget correction or failing product journey
is absorbed. The native cache caller can use these hooks to retain actual
parse/check/expansion attribution rather than hiding combined checking inside
cache lookup or running a dummy parse. Native integration and controlled
remeasurement remain open under #6180. The prior source-pinned attribution
family stays at 83c25d315. This API alone is not an optimization claim.
