# Exact-endpoint isolated HTTP Base

Status: production-selectable Linux profile and adversarial proof for #3089
under epic #3069. Entrance: `cargo xtask check isolated-http-base`.

The authored operation remains `http/client`. Planning selects
`std/isolated-http-client-http1@1`; endpoint, provider, and operating-system
facts do not enter the Form. A minimal Host does not install the profile. A
target explicitly supplies the provider executable, Base instance and
generation, authored HTTP authority, exact resolved socket address, and
resource generation.

## Trusted seam

The narrow bootstrap connects one exact configured endpoint, verifies the
peer, and transfers only that connected TCP descriptor as descriptor 3. The
provider validates the #3072 capability immediately before writing bytes. Its
scope is exact to Host, Boot, Base generation, Plan, active Play,
implementation, operation, HTTP subject, resource pool and generation, and a
single bounded request/response envelope.

Before readiness the provider clears its environment, caps address space at
128 MiB, CPU at 2 seconds, descriptors at 8, processes at 1, and output files
at 1 MiB. It then applies an empty Landlock filesystem ruleset and seccomp
denials for socket/socketpair, process creation/execution, and tracing. It can
read and write its inherited exact-peer descriptor but cannot create an
alternate socket or listener. Credentials, proxy configuration, resolver
state, and unrelated descriptors are not inherited.

The selected endpoint is already an exact DNS result for the current resource
generation; the provider performs no DNS lookup. A changed resolution requires
new resource truth, provider generation, planning, and capability issuance.
HTTP redirects are returned as response data and are never followed. TLS is
not claimed by this HTTP/1.1 profile.

## Finite work and failure

One provider owns one connection, one capability-table entry, one in-flight
operation, and at most eight bounded IPC frames. Semantic encoded request and
response limits are each 32,768 bytes. I/O and connect deadlines are 2 seconds.
Provider loss is failure, never implicit retry or continuation. Replacement
uses a new provider and resource generation and refuses a stale claim.

Patchbay-safe inspection names the Base instance, provider generation,
implementation, descriptive `os-capability-mediated` class, authored
authority, exact endpoint, and resource generation. It exposes no capability
bearer or credential.

## Proof boundary

The executable proof uses an unchanged checked HTTP Form and the normal shared
planner to select the isolated implementation, then binds the operation to an
issued kernel Play. The exact server independently observes one authorized
request. A redirect target/sibling endpoint receives no connection. Forged,
revoked, stale-generation, and wrong-authority operations refuse; raw socket,
listener, and process probes receive kernel `EPERM`.

The trusted bootstrap briefly possesses only the exact connection it passes.
This is not a wildcard network sandbox, TLS authorization, a generic proxy, a
listener Base, or proof for the cooperative legacy HTTP/service clients.
