# Linux hosted Base confinement profile

Status: first hosted process-isolation proof for #3073 under epic #3069.
Entrance: `cargo xtask check hosted-base-isolation`.

## Boundary

The proof launches one file-read Base as a separate process. Trusted bootstrap
provides one exact allowed directory/file and the admitted #3072 capability
scope over a private stdin/stdout channel. The provider creates its private
capability table, applies Linux Landlock, then installs a seccomp filter before
reporting ready or accepting an operation.

Landlock handles the complete version-1 filesystem access set and permits only
read-file/read-directory operations beneath the admitted directory. It denies
raw access to a separately created sibling directory even when hostile provider
code knows the absolute path. Seccomp returns `EPERM` for process creation,
execution, socket/socket-pair creation, and tracing. The launcher clears the
environment, supplies only piped standard descriptors, sets cwd to `/`, and
enforces these rlimits before `exec`:

| Resource | Enforced ceiling |
|---|---:|
| address space | 128 MiB |
| CPU time | 2 seconds |
| open descriptors | 8 |
| processes | 1 |
| output file size | 1 MiB |
| IPC frame | 4096 bytes |
| accepted provider frames | 8 |
| capability table | 1 entry |
| in-flight protected operations | 1 |

The IPC protocol is versioned and deny-unknown-fields structured framing with a
four-byte length prefix. Oversized, empty, truncated, malformed, and unknown
frames refuse before dispatch. Connectivity is not authority: the channel is
bound to one internally held opaque capability, and every read claim is checked
at the provider against exact Host/Boot/Base-generation/Plan/Play/implementation/
operation/subject/resource-generation/envelope facts before `open`.

## Proof

The positive case reads the real allowed file. Negative cases mutate the
subject at the provider boundary, attempt to open the sibling directly through
raw file APIs, spawn a child, and create a network socket. The parent observer
then reads the sibling sentinel independently and proves it is unchanged.
Explicit revocation refuses a later request. A fresh provider process with a
new provider generation and private table refuses the old generation's visible
claim.

The fixture is compiled only with the `isolated-base-proof` feature and is not a
normal std Base offer. The process reports enforcement class
`process-isolated/landlock+seccomp` only after both mechanisms install.

## Trust and non-claims

The launcher and provider bootstrap code before Landlock/seccomp are trusted.
The proof observer owns fixture creation and final sentinel inspection; the thin
Host's normal operation uses only the private IPC channel. This Linux proof does
not establish macOS or Windows parity, a general container runtime, syscall
allow-list completeness, kernel-compromise resistance, physical-device or DMA
isolation, or confinement of existing cooperative std families.

Future macOS providers can map the same universal contract to Sandbox/App
Sandbox plus restricted descriptors. Windows providers can use AppContainer,
restricted tokens, job objects, and exact transferred handles. Those mappings
need their own executed proof before receiving an isolated enforcement class.

## Production file/copy profile

The `isolated-file-base` feature builds the separately installable
`conduit-isolated-copy-base` provider. A target must explicitly construct an
`IsolatedFileHost` with that executable, Base instance identity, and nonzero
provider generation; `StdHostComposition::minimal()` and the ordinary Host do
not install it. Planning keeps the canonical `file/copy` Form and Kind while
selecting `std/isolated-file-copy@1`.

The provider receives only the exact admitted source and destination paths at
trusted bootstrap. Landlock grants read beneath the source directory and the
write/create/remove operations needed for same-directory atomic commit beneath
the destination directory. Seccomp blocks process and socket escape. The
private #3072 capability is bound to Host, Boot, Base generation, Plan, Play,
authority, implementation, operation, subject, composite resource generation,
envelope, and finite chunk count, and is checked before every copy step.

`cargo xtask check hosted-base-isolation` proves the production path as well as
the lower-level fixture. Patchbay-safe inspection reports opaque resource
handles, provider generation, enforcement class, attempt, and terminal result;
it exposes neither paths nor capability bearer material.

The original `std/copy-file@1` realization remains available only as an
explicitly `Cooperative` compatibility profile. It must not be used for hostile
confinement claims. Its removal path is: adopt the isolated provider in shipped
Linux profiles, establish platform-specific equivalents where required, move
the product copy entrance to those profiles, then remove the cooperative offer
and implementation after compatibility evidence shows no remaining consumer.
