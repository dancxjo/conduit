# Implementation confinement and admitted authority

Status: architectural contract and source audit, not hostile-isolation acceptance.
Owner: [#2685](https://github.com/dancxjo/conduit/issues/2685).
Related: [#2682](https://github.com/dancxjo/conduit/issues/2682),
[#2686](https://github.com/dancxjo/conduit/issues/2686),
[#2688](https://github.com/dancxjo/conduit/issues/2688),
[#2690](https://github.com/dancxjo/conduit/issues/2690), and
[#2691](https://github.com/dancxjo/conduit/issues/2691).

## Required property

An implementation must not possess materially more effect authority than its
exact admitted realization. Forms describe meaning; Hosts offer realizations;
Plans select exact current offers, resources, and authority. None of source,
availability, selection, membership, or successful communication grants an
effect permission. Universal computation and continuous lifetime do not change
this rule.

The planner consumes authority facts. It is not an authority issuer. Treat a
forged Plan as attacker input at the enforcement boundary, even when the
ordinary planner would reject it. A protected operation must validate its
current authority against trusted Host-owned state before performing the effect.

## Attacker and trust boundaries

| Boundary | Trusted components | Attacker and required refusal | Present proof limit |
| --- | --- | --- | --- |
| Cooperative std process | Host process, installed native code, OS | Malformed requests, stale identities, incorrect bindings | Semantic checks can refuse requests; native code shares process privileges. No hostile native-code isolation follows. |
| Isolated WASM implementation | Engine, import provider, trusted capability table | Hostile module requests an unprovided import or another subject through a provided import | Requires an actual import inventory and adversarial execution. Compiling Rust to WASM alone is not proof; surrounding JavaScript may possess broader browser effects. |
| ConduitOS implementation | Native kernel/Host protection mechanism and authority table | Implementation attempts direct device or memory access outside its admitted handles | Direction only until memory/device isolation and handle checks are executed and proved. A shared address space is not isolation. |
| Remote Host/Line | Authenticating endpoint and operation enforcement boundary | Forged, replayed, redirected, stale-Boot, or broadened request | Authentication and Line reachability are separate from operation permission. Transport encryption alone does not prove authorization. |
| Physical actuator | Last trusted device driver or hardware gate | Unauthorized command reaches the effect boundary | The mechanism must refuse before actuation; planner refusal and simulated LEDs do not prove a physical gate. |

Denial of service and resource containment are explicit obligations at each
boundary. Do not infer a bound on child-process memory from bounded captured
stdout, or on host allocations from finite kernel queue capacity. Kernel,
engine, adapter, and external work each need their own admitted bound.

## Identity versus possession

`AuthorityGrantId`, HostId, BootId, resource identity, and operation identity are
descriptive values. They may travel in checked evidence and wire messages. An
attacker knowing or copying them must not acquire privilege.

At a hostile boundary, possession must be mediated by a mechanism the attacker
cannot manufacture: for example, an engine-provided import closure carrying a
private capability-table entry, or an OS-protected handle delivered through an
admitted channel. The mechanism references the existing Conduit authority
contract; it does not introduce an independent policy system.

A table entry binds the selected implementation and exact permitted operation,
subject, Host/Boot, resource generation, and finite outstanding-work bounds.
The trusted provider derives that entry from independently validated grants,
not solely from fields supplied by the caller. Handles must not remain usable
after revocation, resource replacement, or Boot replacement. A descriptive ID
lookup that accepts arbitrary caller-created entries is not unforgeable.

## Source audit of std effect paths

Audited against dev `0cf9fbe5f77594cfab572ec9a88b95737d28253e`.
These are concrete effect paths, not an exhaustive transitive dependency or
syscall audit. Test-only filesystem fixtures are not production effect paths.
All native code linked into the cooperative std process must be treated as
trusted until an isolation mechanism establishes a narrower class.

| Effect family | Source evidence | Classification and next proof |
| --- | --- | --- |
| Protected file copy | [copy_task/base.rs](../../targets/std/src/copy_task/base.rs) opens source and temporary files, writes, links or renames the destination | Cooperative-only. Private Rust fields constrain ordinary callers but do not remove native process filesystem authority. Prove denied sibling access at an isolated effect boundary. |
| Executable job | [hosted_job.rs](../../targets/std/src/hosted_job.rs) validates a Resource binding, clears the environment, and spawns an executable | Cooperative-only. Environment clearing and bounded output are useful but do not sandbox child filesystem, network, memory, or descendants. |
| HTTP/network | [hosted_http/mod.rs](../../targets/std/src/hosted_http/mod.rs), [hosted_network.rs](../../targets/std/src/hosted_network.rs) create sockets | Cooperative-only. Exact endpoint policy must reach a provider which cannot be bypassed by the confined implementation. |
| External and planned WebSocket | [external_websocket.rs](../../targets/std/src/external_websocket.rs), [websocket.rs](../../targets/std/src/websocket.rs) acquire socket effects | Cooperative-only. Distinguish an authored external operation from realization of a planned Cord; neither grants arbitrary peer authority. |
| Local model provider | [hosted_local_model/ollama.rs](../../targets/std/src/hosted_local_model/ollama.rs) invokes external commands | Cooperative-only. Provider/process and credential authority need separate admission and confinement proof. |
| Audio and MIDI | [hosted_audio/alsa_aplay.rs](../../targets/std/src/hosted_audio/alsa_aplay.rs), [hosted_midi/output.rs](../../targets/std/src/hosted_midi/output.rs) use child/device effects | Cooperative-only. Device selection and command validation do not establish hostile-code isolation. |
| Device discovery/acquisition | [std_create_uart.rs](../../targets/std/src/std_create_uart.rs), [hosted_midi/discovery.rs](../../targets/std/src/hosted_midi/discovery.rs) inspect devices or launch discovery tools | Cooperative-only. Observation is not permission to acquire or actuate a device. |
| Pure installed transformations | [installed_std.rs](../../targets/std/src/installed_std.rs) composes typed operations with the host adapter | Semantic purity does not by itself isolate the native implementation. Claim impossible sibling effects only for a proved restricted mechanism, not a module name or absence of direct imports. |

## Required executable proof

For the first consequential effect family, run the ordinary checked Form and
Plan through the selected implementation and actual effect provider. The
positive case must produce the exact authorized effect. The negative case must
use an adversarial implementation which attempts an unauthorized sibling
subject or effect, not merely omit a grant at planning time.

Prove forged grant identity, forged Plan fields, wrong operation, wrong subject,
wrong resource generation, wrong Boot, replay where the contract forbids it,
revocation, operation-capacity pressure, cancellation, and terminal disposition.
Observe the protected sibling independently to establish that refusal prevented
the effect. A refusal Sign alone cannot establish that no effect occurred.

For WASM, demonstrate both missing-import refusal and denial through an allowed
import when the caller requests an unauthorized sibling. Record the actual
module imports and memory/work limits. A malicious module must not escape by
requesting another export of the broad Host adapter.

For remote realization, test at the receiving effect provider after decoding
and authentication. Bind current Boot, subject, operation and replay state.
Replacing a Play requires fresh validation; an old handle cannot authorize the
new Boot. Continuity may transfer bounded semantic State, never stale grants.

## Inspection and result semantics

Patchbay and Observatory must distinguish available capability, selected
realization, admitted resource, required authority, granted authority,
enforcement mechanism/trust class, effect attempt, and completed/refused/failed
effect. Displaying a grant ID is not a security badge.

Keep semantic completion, waiting, Lull, cancellation, fuel exhaustion, State
capacity exhaustion, other resource exhaustion, failure, Host/Boot/resource/Line
loss, and Plan retirement distinct. A finite universal Play may exhaust its
embodiment without halting its meaning. Neither exhaustion nor replan grants
additional authority or permits automatic retry.

## Stop line and remaining acceptance

This note establishes terminology, threat assumptions, and an initial source
inventory. It does not implement isolation, claim a completed hostile negative
proof, or close #2685. Mechanism-level effect enforcement, actual authorized and
unauthorized execution, stale-Boot rejection, finite resource enforcement, and
Patchbay projection still require executable acceptance and exact-main evidence.
