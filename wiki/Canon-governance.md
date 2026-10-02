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
