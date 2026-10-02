# Canon governance

The [project canon](https://github.com/dancxjo/conduit/blob/dev/docs/conduit-canon.md)
owns architectural intent and invariants. The handbook's topic references own
the readable language contracts; [[Current language surface|Current-language-surface]]
records implemented spelling and evidence. [STATUS](https://github.com/dancxjo/conduit/blob/dev/STATUS.md)
owns the capability/proof summary, and each active issue owns its task scope.

If those sources disagree, expose the conflict and resolve the relevant wording.
Do not invent an architecture to make them appear consistent.

## Change a language rule deliberately

A language rule changes when:

1. a concrete need exposes the problem
2. an explicit amendment states the new rule in its owning reference
3. contradictory old wording is corrected, rather than leaving two simultaneous canonical choices
4. implementation and migration work is scoped separately where needed

Parser acceptance alone does not establish canonicality. An open proposal does
not become implemented grammar because a learning page uses it. The paired
vocabulary amendment in [#4513](https://github.com/dancxjo/conduit/issues/4513)
was implemented by [#4800](https://github.com/dancxjo/conduit/pull/4800):
executable `form` became `plot`, and representation `code` became `form`.
Current development source uses `plot` and `form`; the former declaration
spellings are rejected. Stable-release acceptance remains a separate claim.

## Review meaning and proof

- Cite the relevant language contract and owning issue
- Keep dimensioned semantic types distinct from a convenient generic carrier
- Distinguish complete source and installed behavior from test-local catalogs and checking fixtures
- Preserve exact bounds, state, effects, terminals, and temporal relationships
- Use `}.` only for intentional completion on structural drain; bare `}` remains live
- Keep a fore's callable signature separate from a face's human meaning
- Keep masks ordinary plots selected through ordinary planning; do not introduce a second UI graph or scheduler
- Preserve historical evidence as evidence of its actual source and environment

If a required rule lacks support, implement only the capability owned by the
current task or report the exact gap. A green narrow fixture is not a target,
physical, human-use, or accepted-release proof.

The repository [working agreement](https://github.com/dancxjo/conduit/blob/dev/AGENTS.md)
and [contribution guide](https://github.com/dancxjo/conduit/blob/dev/CONTRIBUTING.md)
own current collaboration and verification procedure.

## Retain provenance without making it the reading order

Closed design tickets remain useful context. The
[language settlement record](https://github.com/dancxjo/conduit/blob/dev/docs/history/language-settlement.md)
preserves the #4109 implementation audit, recovered decisions, integration gate,
mask-role law, and issue lineage. That dated gate is not a new instruction to
restart its campaign. Current verticals prove the current language under their
own acceptance criteria.

Edit this handbook in repository `wiki/`, through a pull request to `dev`.
The website build renders these pages and their diagrams. Maintain the separate
GitHub wiki manually as a short signpost to the
[main website](https://dancxjo.github.io/conduit/); it is not a handbook mirror
or build input. A draft PR does not publish the website handbook.
