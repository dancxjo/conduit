# Reviewed Linux source builder

`LinuxSourceBuilder` implements `ReviewedBuilderBackend` for trusted repository
source. It supplies real ConduitOS media to `StdBuilderAdapter` and `BuilderBase`;
it does not install an executable Crèche encounter or advertise a runtime offer
by itself.

## Caller admission

The owning Host operation supplies a `CheckedHostConfiguration`, a shared
`Arc<AtomicBool>` cancellation flag, and an explicit local boot-archive read grant:

```rust,ignore
let backend = LinuxSourceBuilder::new(checked_configuration, cancelled)
    .with_reviewed_boot_assets(
        ReviewedConduitosBootAssets::local_limine_archive(admitted_archive),
    );
let adapter = StdBuilderAdapter::new(admitted_source_roots, admitted_origins, backend);
```

The operation must separately admit the source, exact selection, workspace,
output directory, toolchain identity and finite `BuilderBounds` in the normal
builder request. Local source access and an installed compiler do not grant
boot-asset acquisition. Without an explicit boot-archive grant, build realization
refuses `UnauthorizedNetwork` before invoking backend build subprocesses.
The archive pathname is re-read and its bytes verified during staging.

There is no public raw PROFILE, shell-command or executable-override entrance.
The selected checked configuration is serialized as canonical `.host.conduit`
source for the existing repository command:

```text
cargo +stable xtask make host build <selected.host.conduit> --output <directory> --source-identity <sealed-identity>
```

## Supported manufacture

Only `DiskImage` output is supported for these exact targets:

| Target | Installed Rust target |
|---|---|
| `conduitos/x86_64/pc` | `x86_64-unknown-none` |
| `conduitos/aarch64/virt` | `aarch64-unknown-none` |
| `conduitos/riscv64/virt` | `riscv64gc-unknown-none-elf` |
| `conduitos/loongarch64/virt` | `loongarch64-unknown-none` |

Other targets and outputs, including IA32 and SD images, refuse as unsupported.
The ordinary target builder produces ISO media; serialized `image.json` is not
accepted as bootable output.

Toolchain preparation probes installed `stable`, `llvm-tools` and the selected
Rust target. It never installs them, even when a broader toolchain grant permits
acquisition. Cargo runs offline and Rustup auto-install is disabled. The caller
must provide an already available dependency cache and target packaging tools.
The package's current `stable` toolchain label is not an immutable compiler pin;
probe output contributes to the evidence digest.

The local boot archive must be Limine **12.5.2**, SHA-256
`4c760c09c53560d859b362319a3dc63b79cca3d47f35d69ab0106a13b8057055`.
The backend checks the source package's pin and stages the verified archive in
its fresh vendor cache, so the reviewed package path does not need to download
it. There is no boot-package network acquisition grant in this backend.

## Verification and limits

The adapter rejects dirty source. The backend rechecks source identity and builds
from a separate shallow Git clone without local hardlinks. It verifies that
snapshot before and after the build. Output must be newly created, and the
resolved checked image, target manifest, profile/build/source/toolchain bindings,
Limine identity, kernel/media digests, filenames, roles and ISO signature must
agree. Artifact paths cannot traverse parents or follow symlinks. A successful
receipt identifies inert verified bytes; it does not establish boot or join.

Cancellation kills and reaps the command process group. Deadline and evidence
accounting span toolchain preparation and build; source reads and media hashing
check cancellation/deadline between 64 KiB chunks. The backend monitors process
count, workspace/output size and captured output, and limits Cargo job count.

These are cooperative bounds for trusted tooling, not hard disk/process quotas,
network isolation or hostile-code confinement. Monitoring can observe an
overshoot; escaped process groups and blocked OS reads are not contained by this
adapter. Build scripts retain the builder user's authority. The owning installed
operation must retain canonical receipts and drain effects before reporting
terminal cancellation.

Focused tests exercise these contracts. No actual source-to-image build, native
Crèche encounter, emulator boot, physical deployment or Body join has yet been
proved through this backend.
