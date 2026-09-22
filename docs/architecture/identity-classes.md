# Identity classes and stability

Conduit identities name different kinds of truth. A friendly name is not a
version counter, and a version counter is not artifact provenance. New code
must choose the identity class before choosing its spelling.

| Identity class | What it identifies | Stability and exactness |
| --- | --- | --- |
| Kind and Info | portable semantic meaning | Meaningful canonical name; compatibility change belongs in the separate checked contract or profile identity. |
| Resource class | the kind of finite resource required | Meaningful name without an edit counter; quantity and exact binding remain separate. |
| Authority contract | the authority being granted | Meaningful name without an edit counter; the grant binds the exact operation, subject, Host, Boot, and lifecycle. |
| Host Call contract | one platform-effect boundary | Meaningful name without an edit counter; typed subject and finite input/output limits are part of the contract. |
| Base kind | one reusable platform mechanism family | Meaningful name; an initialized Base has separate boot-local identity and resource truth. |
| Execution profile | the realization constraints selected by planning | Meaningful profile name; exact limits and characteristics remain structured data. |
| Implementation | which implementation family realizes a contract | Meaningful implementation name; coexistence requires a genuinely distinct implementation identity, not a source-edit count. |
| Artifact | the executable lineage selected by a plan | `ArtifactId` remains distinct from implementation. Release and fabrication receipts bind actual artifact bytes through their content digest; a friendly artifact label must not be described as a digest. |
| Content/access profile | bounded content meaning and permitted access | Canonical semantic identity; content bytes use their owned digest where identity depends on bytes. |
| Line/base family | one transport or mechanism family | Meaningful family name; exact endpoints, sessions, boot identity, and authority remain separate. |
| Evidence/report schema | retained bytes interpreted by independent readers | Keep an explicit schema family plus a real wire version or discriminator when old and new bytes can coexist. |
| Fabrication/release schema | persisted build, carrier, or release record | Keep explicit version dispatch and content digests. These are genuine compatibility boundaries. |

## Pre-release migration rule

Decorative `@N` suffixes are removed directly from friendly resource,
authority, operation, profile, implementation, and artifact-family names.
There is no alias for an identity that was never a stable external contract.
Real semantic contract revisions, wire-version fields, persisted schema
versions, and release/fabrication versions remain explicit.

The HTTP vertical slice demonstrates the separation. Its resource, authority,
Host Call, execution-profile, implementation, and artifact-family names
are clean. `conduit.http/client@1` and `conduit.http/server@1` remain separate
semantic contract revisions. The isolated provider's `PROTOCOL_VERSION`
remains the decoder discriminator. Plans still bind the exact implementation,
artifact lineage, requirements, limits, Host, Boot, offer generation, resource
bindings, and authority grants; release/fabrication evidence supplies content
digests when claims depend on exact artifact bytes.
