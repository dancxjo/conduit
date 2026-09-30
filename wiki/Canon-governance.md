# Canon governance

> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

## Canon vs implementation status

As of creation of this canon, known mismatches include:

| Area | Canon | Current implementation status |
| --- | --- | --- |
| Form fulfillment | trailing `}.` means structural drain fulfills; bare `}` means drain is quiescence | **IMPLEMENTED:** canonical boundary full stop and live bare boundary are parsed distinctly; historical completion placement and `complete` are rejected |
| Cord / fore direction | `>>` | **IMPLEMENTED:** canonical `>>` is parsed and lowered; legacy single `>` is rejected as a Cord alias |
| keep | `keep T(init) for this …` | **IMPLEMENTED:** checked step/play/wake/boot/body/life durations, optional/bounded keeps and typed State lowering |
| Temporal `$T`, `T...`, `T...|` | as above | parser support exists for established temporal sigils |
| Routing | `value >> ? { pattern: route }` | **IMPLEMENTED:** canonical matched routing uses `>>`; legacy single `>` is rejected |
| Ternary | `condition ? yes : no` | **IMPLEMENTED:** checked precedence, typing and lazy branch execution |
| `when(expr)` | unary pure filter | **IMPLEMENTED:** production checked lowering and conformance proof |
| Optional `T?` / `$T?` | finite optional info | **IMPLEMENTED:** surface/check support for value and current optional ports |
| Cancellation | `gear~` | **IMPLEMENTED:** semantic cancellation projection with declared checked transduction |
| Close/fault projections | `endpoint|`, `endpoint!` | **IMPLEMENTED:** typed normal-close and exact abnormal terminal projections with checked propagation |
| Anonymous records/tuples/projections | section 9 | **IMPLEMENTED:** checked structured expressions, projections and conformance tests |
| Immutable locals | `name = expr` | **IMPLEMENTED:** checked immutable expression bindings |
| Quantities | exact semantic units | **IMPLEMENTED CORE:** exact dimensions and units cross core, Form values/startup and expression paths; living vertical proof may still expose domain gaps |
| Effect/replay inference | sections 19–20 | **IMPLEMENTED CORE:** exact effect/state/time/random/resource/determinism axes are inferred independently; absence never implies purity, and replay of effectful work requires retained provider-owned evidence (#4170) |

This table is intentionally allowed to say **GAP**. A green test that changes only a test-local catalog does not turn GAP into implemented.

---

## Recovered-area implementation audit

The salvage pass added semantic law that was present in closed design tickets but absent or flattened in the original #4109 table. **Canon status and implementation status remain separate.**

| Recovered area | Canon status | Implementation status / living owner |
| --- | --- | --- |
| Deep semantic paths + fore vocabulary | **canonical** | **IMPLEMENTED:** recursive checked/expanded paths and Fore vocabulary are covered by current API and tests |
| data / `&T` / save / load / disk | semantic law + `&T` spelling **canonical** | **vertical owner #4116; implementation audit required** |
| Fixed-width integers, base literals, Boolean/bitwise ops, `<<<` / `>>>` | **canonical** | **IMPLEMENTED:** checked literals, fixed integer values, typed expression evaluation and conformance tests |
| Terminal transduction / default abnormal propagation | **canonical semantic law** | **IMPLEMENTED CORE:** checked configuration through Form lowering, planning and kernel propagation; verticals still prove domain-specific policies |
| Cord fan-out pressure/delivery law | **canonical semantic law** | **IMPLEMENTED CORE:** exact delivery policy, planner sealing and atomic external-Fore fan-out are covered by conformance tests |
| Realization invariance | **canonical planner/checker law** | **IMPLEMENTED CORE:** offers and Plans retain exact semantic contracts and Host admission fails closed (#4159, #4189); each living vertical still owns proof that its concrete substitution family is genuinely invariant |
| Pack identity/resolution/distribution laws | **canonical semantic/ecosystem law + frozen authored surface** | **IMPLEMENTED:** `with`, `sans glyphs`, `pack.conduit`, `ship`, `need` and generated exact `conduit.lock` truth are checked finitely; legacy spellings refuse; #4055 is complete |
| ordinary Mask Form role + wear/doff/preference | role Fore + semantic law + authored wardrobe **canonical** | core wardrobe/control exists; source converges on `wear` / `else` / `want` and runtime `doff` under #4115 |
| `host` / `body` / `pack` Conduit document roles | **canonical** | existing host/body source plus frozen minimal Host grammar and pack source; convergence under #4117/#4055 |
| Low-level resource/capability port surface | semantic authority law + `resource T` spelling **canonical** | core resource identity/checking/Plan lowering exists; source converges under #4065/#4117 |
| Natural-duration hard-loss proof | **canonical meaning** | living proof owner #4116 |
| Direct/recursive mask realization + highest-honest seam | **canonical architecture** | living owner #4115, native proving owner #4117 |
| Same-plan admitted fallback vs replacement planning | **canonical control-loop law** | living distributed proof #4092; mask proof #4093/#4115 |

No row may be promoted from “audit required,” “gap,” or “unfrozen” merely because a narrow fixture can be made green.

---

---

## Rules for agents and reviews

Every Conduitese-changing PR must obey:

1. **Cite this issue as canon.**
2. Do not treat parser acceptance as proof of canonicality.
3. Do not introduce legacy `>` Cords in new canonical source.
4. Do not replace a dimensioned semantic contract with generic `Quantity` merely to reuse representation.
5. Do not fabricate missing production kinds/contracts solely in a test catalog and then claim the language supports them.
6. If canon is unsupported, either:
   - implement the smallest general missing capability clearly owned by the assigned task; or
   - stop and identify the exact canon gap/blocker.
7. New syntax requires an explicit amendment to this canon. It does not become language law because one PR happens to parse it.
8. Historical source/evidence remains historically accurate; migration applies to current canonical source.
9. Tests must distinguish **canonical language conformance** from narrow unit fixtures.
10. Review comments directed to the coding agent should explicitly mention `@copilot`.
11. Do not emit `}.` casually: it changes lifecycle semantics. Use it only when the form is intentionally finite-on-drain; otherwise end the form with bare `}`.

---

---

## Current vertical use

The active verticals are the proving grounds, not alternate language specifications:

- #4090 Browser / Field Station
- #4091 Pocket Theremin
- #4092 distributed / two-model replan
- #4093 Three Bodies

For #4091 / #4097 specifically, canonical source must not silently regress to legacy `>`, generic `Quantity -> Quantity`, or fixture-only `current/keep` merely because those are easier to make green.

PR #4103 is a concrete example of why this canon exists.

---

---

## Amendment law

This issue is intended to stay open.

A language rule changes only when:

1. a concrete need exposes the problem;
2. the new rule is written explicitly here (or in an explicitly designated successor canon);
3. contradictory old wording is edited rather than left as two simultaneous “canonical” choices;
4. implementation/migration work is ticketed separately if needed.

Small implementation tickets close. This canon does not close merely because one implementation catches up.

---

---

## Provenance ledger

The canon above consolidates accepted decisions and deliberate unresolved points from:

#3938 #3939 #3955 #3956 #3964 #3967 #3968 #3969 #3970 #3975 #3977 #3999 #4001 #4002 #4044 #4046 #4048 #4049 #4050 #4052 #4053 #4059 #4060 #4062 #4064 #4065 #4066 #4070 #4071 #4072

Closed provenance issues remain useful historical/design context, but this issue is the single living authority.

---

## Implementation gate for current integration work

For the next integration pass, this canon is a **gate**, not a reference document to cherry-pick from.

Order of work:

1. finish/merge the already-open integration PRs so we begin from one coherent tree;
2. implement the **frozen, normative** language surface and checked semantics in this issue as one coherent Conduitese vertical;
3. update parser/checker/lowering/runtime-facing contracts, examples, fixtures, diagnostics, and representative production Forms together so they agree on the same canon;
4. expose any genuinely blocked or still-UNFROZEN surface as a blocker instead of inventing syntax or retaining a legacy spelling;
5. only then repair the flagship Three Bodies Journey (#3529 / #4082) against the resulting canonical language.

The Three Bodies repair must not become a compatibility shelter for pre-canon syntax. If the Journey only works by using a spelling or semantic shortcut that this issue rejects, **fix the implementation first**.

For this pass, “implement #4109” means: every section that is frozen/normative and has an implementation obligation is either implemented end-to-end or has an explicit named blocker. Sections explicitly marked **UNFROZEN** remain intentionally unimplemented as authored syntax until this canon freezes them.

Exit condition for the gate:

- canonical examples parse/check according to this issue;
- legacy spellings called out here are rejected or migrated as specified;
- production examples/fixtures no longer quietly teach a conflicting language;
- implementation-status notes in this issue can be updated from GAP/PARTIAL to implemented, or to a concrete blocker;
- downstream showcase work can rely on one language instead of compensating for several historical dialects.

---

## Face realization boundary: Masks

Body/application Forms must not absorb Face-realization plumbing merely because the plumbing can be represented as Gears. Ordinary Forms serving the Mask role own that realization meaning.

Canonical architecture is owned by #3951:

~~~text
Body -> Face -> Mask Form -> Show
~~~

Therefore a Body/application Form should not ordinarily contain chains such as:

~~~text
LLM -> TTS -> PCM playback
layout -> compositor -> framebuffer
~~~

when those operations exist solely to realize the Face for a medium.

Those belong to an **ordinary Form serving the Mask role and its exact ordinary Plan**.

#4115 owns the Mask Form role, runtime wardrobe and flagship proof.

### Status: Mask Form role and Body wardrobe grammar are canonical

Mask topology uses ordinary canonical `form` source, checking, expansion, planning and execution. A representative Mask-role Fore has one bounded face input (currently carried by the migration-era Rust `Presentation` type) and `FaceInteraction` plus `Show` outputs. There is no `mask` keyword, second graph language or Mask-specific planner.

Required semantic direction:

- a Mask consumes/correlates one bounded Face revision and produces one Show per realization path;
- one Mask may contain multiple heterogeneous typed realization stages;
- exact implementation/provider/device/Host/resource selection remains Plan/Back truth;
- Mask-local intermediate values do not become Face truth;
- local input returns through the Mask as an exact Face interaction;
- the same Face may be realized by several independently planned Masks.

Do not invent production `mask` grammar. Admit the role from an ordinary Form's exact Fore, keep application Forms distinct from Mask Forms by role/ownership, and keep all internal stages as ordinary typed Gears and Cords.

The ordinary Mask Form role is incorporated into this canon and the current implementation gate. Body wardrobe source uses canonical `wear`, `else` and `want`; runtime wardrobe control uses `wear` and `doff`. Explicit `else` admits fallback into the Plan; absent `else`, replacement requires ordinary replanning. See #4115.

---

---

## Living verticals as conformance gates

Closed design tickets are provenance. **Living verticals are where the language earns its promises.**

Current conformance map:

~~~text
#4090  Browser Clock
       source Gear/time semantics, Body/Host/Boot/Plan/Play identity,
       browser effect ownership, reload/recovery truth

#4091  Pocket Theremin
       quantities, keep/current control, Cord pressure, bounded audio,
       cancellation/terminal truth, Back substitution

#4092  Two Ollamas
       realization invariance, effect/replay law, same-Plan fallback,
       replacement planning, causal evidence

#4093  Three Bodies
       Face -> Mask Form -> Show, independent births,
       runtime wardrobe, truthful cross-embodiment documentary

#4116  Durable Notebook
       finite defaults, keep duration, data/&T, save/load,
       hard-loss recovery, memoization boundary

#4117  Bare metal to Show
       profile/build/image, Host source/profile, runtime resource ports,
       machine membrane, Forms-all-the-way-down, recursive Mask realization
~~~

The pack/import, variant, generic type-parameter, Body wardrobe, resource and minimal Host surfaces are now frozen above. Living verticals pressure-test implementation and may expose future explicit amendment needs.

A vertical may expose a language gap. It may not invent private syntax to conceal it.

Provenance umbrella: #3966, #3978, #4043.

---

## Do not invent proving work merely to cover a feature

The vertical map above is intentionally **not** a one-feature-one-demo checklist.

The formerly-unfrozen authored surfaces are now settled by the 2026-09-28 amendment:

- finite variant construction/patterns: #4002;
- Gear glyphs and the standard opt-out glyph prelude: #4335;
- merge/race/zip/combine-latest/current-sample glyphs: #4046 #4050 #4064;
- named generic `type` parameters: #4059;
- `with`, `pack.conduit` and `conduit.lock`: #4055;
- checked `~ /.../flags` and `!~ /.../flags` refinements: #4199;
- `resource T`: #4065;
- Body `wear` / `else` / `want` and runtime `doff`: #4115;
- minimal Host construction grammar: #4117;
- explicit Current sampling for publication: #4064 #4116.

Living verticals must prove these surfaces rather than invent replacements. Any future change requires the amendment law below.
---

## Recovered provenance map

Closed issues remain historical quarry. The live canon above carries their durable law.

### Language surface and checked meaning

#3938 #3939 #3954 #3955 #3956 #3958 #3964 #3966 #3967 #3968 #3969 #3970 #3971 #3972 #3973 #3975 #3976 #3977 #3978 #3998 #3999 #4000 #4001 #4002 #4014 #4043 #4044 #4045 #4046 #4047 #4048 #4049 #4050 #4051 #4052 #4053 #4054 #4055 #4056 #4057 #4058 #4059 #4060 #4061 #4062 #4063 #4064 #4065 #4066 #4070 #4071 #4072

### Vocabulary and architectural altitude

#610 #615 #644 #1281 #1519 #1752 #1780 #2284 #3717 #3718 #3724 #3831 #3862 #3863 #3865 #4037 #4066

### Face / mask / show ancestry

#602 #873 #884 #892 #1154 #1155 #3224 #3397 #3524 #3528 #3708 #3951 #3974

### Host fabrication and native-systems ancestry

#1137 #1138 #1139 #1170 #1173 #1174 #1752 #1780 #2284 #3518 #3526 #3993 #3994 #3995 #3996 #4012

### Planning, fallback and causal recovery ancestry

#554 #560 #2888 #3613 #3710 #4001 #4072

These groupings are intentionally by **current concern**, not by ticket chronology. A closed issue is not a second authority and need not be reopened merely to preserve its good idea.
