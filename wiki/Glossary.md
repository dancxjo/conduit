Conduit uses ordinary nouns very deliberately, but they are easier to learn in **small semantic clusters** than as one ceremonial procession.

## Information

| term | meaning |
|---|---|
| **type** | reusable semantic contract describing what values mean |
| **form** | one concrete portable representation of a type; not the type's meaning itself |
| **info** | one finite typed/structured value carried through cords |
| **data** | independently addressable immutable content generation |
| **&T** | ordinary finite info naming one exact data generation containing `T`, not authority or a pointer |
| **keep** | retained current semantic value with an exact duration |

A type may have more than one form. A current value in a keep may change while an earlier saved data generation remains immutable.

## Work and realization

| term | meaning |
|---|---|
| **kind** | reusable semantic contract for executable work |
| **fore** | stable checked outward/callable boundary of a kind or plot |
| **back** | one concrete realization behind a compatible fore |
| **gear** | one configured occurrence of a kind in a plot |
| **port** | typed directional semantic point on a gear/fore |
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

A face is not a widget tree, and a show is not authoritative merely because it is visible.

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
type → form

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

## Vocabulary migration

The current vocabulary is deliberately moving toward:

```text
historical executable form  → plot
representation              → form
```

The semantic reason is that **plot** better names an authored course of events, while **form** better names the concrete shape of a value.

Exact implemented syntax may lag the conceptual vocabulary during migration; [[Current language surface|Current-language-surface]] is authoritative for what the parser accepts today.

## Superseded vocabulary

Historical repository material may use older names.

| historical | current |
|---|---|
| generic architectural **Data** | **info** |
| callable **Face/Front** | **fore** |
| executable **Form** | **plot** |
| representation contract | **form** |
| body human-facing semantic surface | **face** |
| Presenter | **mask** |
| presentation Manifestation | **show** |
| HostOperation | **host call** |
| Seed as privileged body semantic identity | no privileged seed; ordinary plots/workset participate in birth |

The current noun **data** was later reintroduced with the narrower meaning “independently addressable immutable content generation.”

Do not blindly rewrite historical proof or receipts. Old evidence should keep the words it actually recorded.
