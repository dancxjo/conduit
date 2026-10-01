> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

## Vocabulary

In prose, these are ordinary lowercase nouns:

`form`, `body`, `info`, `kind`, `gear`, `port`, `cord`, `fore`, `back`, `host`, `plan`, `play`.

Use capitalization normally at sentence starts and for actual proper names/code identifiers such as `Conduit`, `ConduitOS`, `Patchbay`, `BodyId`, or Rust `Form`.

Provenance: #4066.

---

---

## Semantic paths and the Fore boundary

Semantic Kind paths are **arbitrarily deep cladistic names**, not fixed-rank `category/leaf` tuples.

~~~text
machine/memory/mmio/read/u32
bus/pci/function/bar
audio/hda/codec/verb
math/matrix/multiply
~~~

Common prefixes may express organization/ancestry, but **path depth and prefix do not establish type compatibility**. Exact semantic compatibility comes from reviewed Kind identity/contract and Fore law.

Canonical grammar direction:

~~~text
KindPath := Name ("/" Name)+
~~~

Apply ordinary identifier/source-size bounds rather than a small aesthetic depth limit.

**Fore** is the canonical architecture noun for the stable checked callable boundary of a Kind/Form:

~~~text
Kind = semantic meaning
Fore = how the Kind/Form is called
Back = one realization behind that Fore
Face = the Body's human-facing semantic encounter and control grammar
~~~

A Kind/Form has a Fore and a Back. A Body has a Face.

`Fore` is architecture/spec vocabulary; it need not appear as an authored keyword. The parenthesized Form boundary is its Fore.

Shared Fore shape alone does not make different Kinds substitutable.

Provenance: #3998, #4037.

---

---

## Architectural altitude and superseded vocabulary

Keep each noun at one altitude:

~~~text
Kind      semantic contract: what something means
Fore      stable checked callable boundary of a Kind/Form
Back      one realization of that Kind/Fore
Gear      one configured occurrence/invocation of a Kind in a Form
Port      typed directional semantic point on a Gear/Fore
Cord      semantic connection between compatible Ports
info      general typed/structured value carried through Cords
data      independently addressable content generation (section 6A), not the generic payload noun
Base      concrete mechanism/resource boundary beneath a Host
Line      finite connectivity realization
Sign      bounded evidence/observation about what is true or what happened
Host      truthful finite current realization offers for one Boot
Plan      exact immutable selected realization
Play      active execution of one exact Plan
Step      one bounded kernel transition while advancing a Play
Host Call one bounded request an executing Back makes across the Host boundary
Body      durable continuant
Face      Body's human-facing semantic encounter and control grammar
Mask      planned user-agent realization of a Face
Show      one finite realized occurrence through a Mask
~~~

Compact execution sentence:

> **A Gear invokes a Kind through its Fore. A Host offers a Back for that Kind. A Plan chooses the Back. A Play advances it in Steps. A Step may make a Host Call.**

Compact flow sentence:

> **Gears exchange info through Cords; Lines may realize cross-Host carriage; Signs tell planning/execution what the realized world establishes.**

Lifecycle action vocabulary uses **birth** for the explicit authorized action that creates a Body. A birth event/Sign is evidence of that transition. birth creates the continuant; it does not implicitly Wake, Plan or Play. WAKE and LULL remain Body lifecycle concepts distinct from Boot and Play.

### Supersession map

Historical tickets may contain earlier nouns. Current meaning is:

~~~text
generic architectural Data     -> info
old semantic Face/Front        -> fore (callable Kind/Form boundary)
Body human-facing semantic surface -> face
Presenter                      -> mask
presentation Manifestation     -> show
HostOperation                  -> host call
BE BORN / BORN action label    -> birth
~~~

This is semantic migration, not blind text replacement. Historical evidence keeps the words it actually recorded.

The reintroduced current noun **data** is intentionally narrower than the retired generic Data noun: data now means independently addressable content under section 6A.

Do not use Capability as a generic synonym for implementation/offer. Capability retains possession/authority meaning where appropriate.

Provenance: #610, #615, #644, #1281, #3831, #3862, #3863, #3865, #4037, #3951.

---
