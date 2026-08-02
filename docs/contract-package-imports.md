# Contract-package imports

Panel imports bind source names to published semantic contracts. They do not
locate or install implementations.

```panel
import conduit.dev/std/{tee, gate as valve}
import knowledge.dev/retrieval as knowledge
import example.dev/parts as parts

branch: tee
permit: valve
lookup: knowledge
inspect: parts.probe
```

`tee`, `valve`, `knowledge`, and `parts.probe` are local source names. A
successful resolution records the package-owned canonical identity and exact
descriptor hash for each name. Aliases never become semantic identities.
Direct use of a canonical identity remains a checker/catalog concern; it does
not bypass the lock or make a provider available.

## Identity and ownership

A current pre-release canonical identity is URI-shaped, unversioned, and owned
by its package namespace, for example `conduit.dev/std/tee`. It is an
identifier, not a URL or location. Conduit never dereferences it. A matching
HTTPS documentation location or an artifact mirror does not change its
meaning.

Contract-package and lock artifacts use draft marker `0`, consistently with
the pre-release policy. Package identity, export identity, descriptor hash, and
artifact byte digest are separate facts:

- the package identity owns the public namespace;
- the export identity names one semantic type, node, composite, interface, or
  adapter;
- the descriptor hash pins that exact current semantic contract;
- the artifact digest pins the supplied immutable package bytes;
- the mirror is provenance only and is absent from semantic identity.

Changed repository-owned draft bytes replace the current artifact and lock.
There is no version range, fallback reader, alias universe, or second accepted
pre-release generation.

## Offline resolution

`resolve_package_imports` receives a parsed Panel, checked lock data, and
caller-supplied immutable artifact bytes. Its API has no loader, network,
filesystem, process, prompt, enrollment, grant, installation, provider
registry, or execution capability. It:

1. validates the single current draft marker and exact artifact digest;
2. validates package ownership, complete export pins, and transitive package
   pins;
3. exposes only exports marked public;
4. detects duplicate, ambiguous, missing, hidden, and drifted imports
   deterministically with source spans;
5. rewrites used local source names to canonical identities in a cloned
   semantic Panel while retaining the authored import declarations;
6. returns alias, canonical identity, package identity, and descriptor hash as
   separate checked facts.

Identical bytes supplied through different mirrors resolve identically.
Mutated bytes under the same claimed package are rejected by the artifact
digest before descriptor resolution.

## Separation from planning and acquisition

The four boundaries are explicit:

1. import resolution binds source names to supplied semantic descriptors;
2. package and lock data tell the checker which descriptors are known;
3. provider resolution later evaluates installed implementations, host facts,
   grants, resources, compatibility proofs, and exact-plan rules;
4. acquisition or enrollment is a separately authorized action outside
   parsing and import resolution.

An imported node may therefore check as `contract-only`. Import success does
not imply host support, artifact selection, permission, authority, or an
executable provider. Exact topology and plans retain the canonical contract ID
and descriptor hash. Exact node evidence repeats those facts, and Patchbay may
show the friendly alias beside them without presenting the alias as identity.

Structural compatibility remains the behavior defined by Conduit's
TypeContract, PortContract, and interface satisfaction engine. A
foreign-owned descriptor can satisfy a structural requirement without a
nominal `implements` declaration; importing either descriptor does not alter
that proof or collapse its ownership.

## Stable diagnostics

| Code | Meaning |
|---|---|
| `CND-IPK-001` | malformed or non-current package/lock form |
| `CND-IPK-002` | duplicate, collision, or ambiguous target |
| `CND-IPK-003` | locked artifact bytes absent |
| `CND-IPK-004` | missing package, export, or transitive package |
| `CND-IPK-005` | artifact dependency or descriptor differs from the lock/checker |
| `CND-IPK-006` | requested export is private |
| `CND-IPK-007` | imported export kind is invalid at the source use site |
| `CND-IPK-008` | package count, byte closure, export, or dependency bound exceeded |

The plain-language rule is: imports name the part; the plan determines whether
a particular machine can provide it.
