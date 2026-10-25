# Scoped filesystem Base

Owner: #6440. This is a read-only, Linux trusted-provider conformance seam;
#6193 still owns an installed namespace Back and editor integration. No native
product, semantic Kind, Type, filesystem format, or scheduler is introduced.

## Current providers and proof boundaries

| Owner | Actual operations and guarantees | Executable check and limit |
|---|---|---|
| `targets/std/src/copy_task/{registry,base}.rs` | Private protected source/destination choices, bounded copy chunks, create-only link or replace rename, temporary-file sync. Not a directory namespace; directory durability is not promised. | `copy_task` tests; hosted cooperative boundary. |
| `targets/std/src/isolated_copy_base/` and `isolated_base.rs` | Exact protected copy/read authority, Landlock and seccomp confinement in the selected Linux process profile. | `isolated_file_copy` and `isolated_base_security`; actual Linux mechanism, not a general namespace front. |
| `targets/std/resource-snapshot/src/{directory,snapshot}.rs` | Retained directory descriptor, immutable DATA generations and publication. Explicitly not a Host offer or arbitrary file namespace. | resource-snapshot package tests; immutable-generation contract, not editor directory membership. |
| `targets/browser/host/assets/browser-{application,resource}-storage.mjs` | Application state and resource generation persistence in browser storage. No user filesystem namespace or arbitrary pathname authority. | Browser storage contract/proof suites; browser storage is not POSIX durability or native file access. |
| `targets/conduitos/src/storage_base/` | Checked block-read requests, exact Root-owned block authority and finite transaction preparation/settlement. Block DISK and DATA differ from directory entries. | `storage_base` native contract/owner tests; no directory namespace, file open/create/replace, or filesystem format claim. |
| `targets/std/src/scoped_namespace/` | Preparation-time finite direct-child membership; exact current root/provider/Boot/Plan/Play possession; relative nonfollowing identity checks and bounded retained-file reads. | `scoped_namespace` deterministic Linux descriptor-boundary tests; cooperative process, not hostile native-code confinement. |

## Selected realization and possession

The existing `BaseRegistry` projects Ready capability/resource offers together
with exact provider provenance. The normal checker/planner seals one
`BaseProviderBinding`, resource pool and authority binding into the selected
gear. The trusted Host admits the active Play and supplies independently current
authority; the Plan, entry ID and public authority IDs are descriptive.
`ScopedNamespace::issue` verifies the sealed fragment and exact selected gear,
then narrows the existing `BaseCapabilityTable` authority. A provider's caller
must never manufacture its authority ceiling from an untrusted Plan.

The root is an existing `ResourcePoolId` plus `ResourceGenerationId`, privately
associated with one retained directory descriptor. Physical paths never enter
plots, offers or entry identities. `ResourceEntryId` identifies a direct-child
membership in that root; `display_name` is only a name. Neither grants access.
The listing is finite membership observed during preparation, not a guarantee
that a live directory was atomically sampled. Preparation retains each inode (and a read-only descriptor for each regular file); this prevents inode-number reuse from reviving stale membership. Open checks device/inode again;
replacement refuses. A rename of the root's old pathname does not retarget the
retained descriptor. Symlinks cannot be followed. External hard-link aliases
and mutable file contents remain possible: root scoping describes name lookup,
not exclusive ownership of an inode or an immutable content generation.

Only list and open/read are supported. Create, replace, transaction commit,
atomic file snapshots, sync and durability are unavailable. The provider does
not advertise unsupported effects, and an unsupported operation refuses.
Entry/name/read/descriptor/capability/operation limits are explicit. The descriptor ceiling admits the retained root, one inode per entry and up to two transient preparation descriptors. Linux preparation requires `/proc/self/fd` to reopen an already-resolved regular inode safely. Directory enumeration,
name storage and descriptor ownership are prepared before Play. Reads use
caller-admitted output storage; transient open ownership is bounded to one
operation. Refusals publish no partial bytes. This does not claim real-time
filesystem latency, physical capacity reservation, or a hostile-code sandbox.

A fresh Boot or provider generation requires a fresh owner and private issuer;
old possession is unknown there. The Host revokes the old Play before retiring
its owner. Semantically continuous document identity does not transfer bearer
possession, directory membership or content generation.

## Conformance and remaining product work

The `scoped_namespace` test uses a named fixture-only Kind with no installed
semantic vocabulary, passes its Base-owned offer through the ordinary planner,
and performs real Linux descriptor listing/read boundaries. No existing list/open namespace Fore exists in the current catalog: `file/copy` has different meaning, while `data/load/text` names immutable DATA generations. The synthetic Fore stays entirely in test scaffolding. It tests root
rename, wrong scope, another issuer's possession, revocation, finite slots,
symlink/traversal refusal, replacement and byte overflow with unchanged external
sentinels. This is not a product namespace Back. A second fixture lowers the selected Plan into the production scheduler, dispatches its admitted Host Call to this same boundary, and preserves successful List/Open and explicit provider-generation refusal.
The selected product front and its Host Call dispatch remain #6193 work; do not
substitute the fixture for an installed editor/namespace journey.
