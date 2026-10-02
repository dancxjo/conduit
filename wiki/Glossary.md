Conduit uses ordinary nouns very deliberately, but they are easier to learn in **small semantic clusters** than as one ceremonial procession.

## Information

| term | meaning |
|---|---|
| **type** | reusable semantic contract describing what values mean |
| **form** | an exact portable representation contract, distinct from type meaning |
| **info** | one finite typed/structured value carried through cords |
| **data** | independently addressable immutable content generation |
| **&T** | ordinary finite info naming one exact data generation containing `T`, not authority or a pointer |
| **keep** | retained current semantic value with an exact duration |

type meaning and form identity are separate. A current value in a keep may change while an earlier saved data generation remains immutable.

## Work and realization

| term | meaning |
|---|---|
| **kind** | reusable semantic contract for executable work |
| **fore** | user-visible checked callable signature of a kind or plot |
| **back** | one concrete realization compatible with a fore |
| **gear** | one configured occurrence of a kind in a plot |
| **port** | typed directional point named by a fore and used by gears |
| **cord** | semantic connection between compatible ports |
| **line** | finite concrete carriage that may realize a cord across hosts |
| **base** | concrete mechanism/resource boundary beneath a host |
| **resource** | admitted runtime possession required by realization |

A cord is not a line, and a kind is not its back.

## Authored execution

| term | meaning |
|---|---|
| **plot** | authored portable course of semantic work: what should happen |
| **plan** | exact immutable admitted realization of a plot under current truth |
| **play** | active execution of one exact plan |
| **step** | one bounded kernel transition during a play |
| **host call** | one bounded request from an executing back into host machinery |
| **sign** | bounded evidence/observation about truth or events |

A changed plan does not imply a changed plot. A sign is evidence, not authority.

## Continuity

| term | meaning |
|---|---|
| **body** | durable logical computer that can continue while machinery changes |
| **part** | durable membership relationship inside a body |
| **host** | current realization environment capable of offering backs |
| **boot** | one current incarnation/generation of a host |
| **birth** | explicit authorized creation of a body |
| **wake** | body lifecycle transition into active availability |
| **lull** | body lifecycle transition out of active play while preserving continuity |

A host can reboot without becoming a new body; a body can lose a host without necessarily losing its identity.

## Human encounter

| term | meaning |
|---|---|
| **face** | body's human-facing semantic encounter and agency |
| **mask** | planned user-agent realization of a face |
| **show** | one finite concrete manifestation through a mask |

A face is not a widget tree, and a show is not authoritative merely because it is visible. **face is not a synonym, ancestor, or presentation-side version of fore; the concepts are unrelated.**

## Source and system nouns

| term | meaning |
|---|---|
| **pack** | authored source/dependency shipment unit |
| **Conduitese** | human-authored language of `.conduit` source |
| **ConduitOS** | freestanding Conduit host environment |
| **Patchbay** | inspection/workbench projection over authoritative Conduit truth |
| **Crèche** | zero-body/birth entrance and body-creation experience |

## The compact map

```text
type → info (carried through a form)

kind → fore → back
          ↓
        gear → port → cord → line

plot → plan → play

body → wake / lull
  ↓
 hosts / boots / parts

keep ↔ current truth
data ↔ immutable named generation

face → mask → show
```

The arrows are mnemonic, not a claim that every noun is a pipeline stage.

## Current vocabulary

Current development source uses executable `plot` and representation `form`:
`type → form` and `plot → plan → play`.
[#4800](https://github.com/dancxjo/conduit/pull/4800) implements the paired
migration from [#4513](https://github.com/dancxjo/conduit/issues/4513). Former
executable `form` and representation `code` declarations are rejected. Follow
[[Current language surface|Current-language-surface]] when writing source.
