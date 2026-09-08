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
