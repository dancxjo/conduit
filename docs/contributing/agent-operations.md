# Local environment stewardship

These operational rules apply to agents maintaining a workspace. New contributors
can start with [CONTRIBUTING](../../CONTRIBUTING.md); this is not a setup checklist.

Agents must assess the actual environment, ownership, active processes, and storage pressure when deciding when and how to clean. A `.local` hostname suffix is not required and does not by itself establish ownership.

On Dan's machines `forebrain`, `victus`, and `envie` (including their fully qualified hostnames), agents are responsible for proactively maintaining usable disk space. Check headroom before substantial builds and recover space from verified disposable artifacts when needed; do not wait for disk exhaustion or ask again for routine cleanup permission. This responsibility does not authorize deleting uncommitted work, unique evidence, or personal data. These hosts own their workspaces; agents may manage workspace lifecycle according to the host's local policy.

In cloud, CI, hosted runner, and other managed environments, the agent's own environment-specific rules, permissions, and cleanup lifecycle govern. This repository imposes no blanket prohibition on cloud cleanup and grants no additional privilege there. Follow those environment rules for workspace and cache retention, shared resources, and disposal. If the environment or ownership is uncertain, investigate and limit cleanup to verified disposable artifacts within the agent's authority; report any remaining blocker.

On user-owned machines, recover space in this order:

1. Inventory filesystem usage and the largest directories before changing state. Check for active Cargo, compiler, browser-proof, VM, and other processes that may own candidate artifacts.
2. Remove regenerable build outputs such as inactive Rust `target` directories and tool caches. Preserve outputs used by a running process. Do not delete source trees, repositories, credentials, downloads, virtual-machine images, or other user data merely because they are large.
3. Empty desktop trash and remove stale user-owned temporary artifacts. Do not disturb active sockets, sessions, system-owned temporary paths, or recent artifacts whose ownership is unclear.
4. Use package-native cleanup for package caches and bounded journal retention when available. Do not bypass missing privileges or turn a cleanup into an operating-system reconfiguration.
5. Report the before/after free space, what classes of data were removed, what large candidates were deliberately preserved, and any cleanup blocked by permissions.

Disk cleanup is machine maintenance, not permission to change Conduit source or enlarge an issue's implementation scope.
