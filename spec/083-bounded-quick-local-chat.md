# Bounded quick-local chat

Status: current pre-release contract.

This specification defines the first `conduit.ai` domain operation. It uses
the generic implementation, artifact, host-observation, capability, authority,
exact-plan, runtime-evidence, and presentation boundaries from specifications
031, 032, 079, and 082. It does not define an AI-specific resolver.

## Requirements

### CHAT-001 — ordinary semantic boundary

`ai/chat` MUST be an ordinary node with required `message: std/text`, optional
caller-supplied `context: ai/chat/context`, bounded `chunks: std/text`, and a
typed `result: ai/chat/result`. HTTP, process, model framework, endpoint, and
credential types MUST NOT appear in these ports.

### CHAT-002 — exact limited mode

The current mode is `conduit.ai/chat/quick-local`. It means text-only,
caller-supplied context, finite input/context/output/chunk limits, no retained
conversation state, public sensitivity, no tools, no structured-output
guarantee, local processing, and a declared latency objective. “Quick” is a
compute/latency profile, not a universal intelligence or quality score.

### CHAT-003 — one generic implementation path

Every provider MUST install through `InstalledImplementationRegistration`.
Several hosts and several provider technologies MAY implement the same exact
node contract. Contract registration alone MUST remain honestly
`contract-only`; installation MUST NOT create a host observation, capability,
resource, grant, or authority.

### CHAT-004 — closed provider facts

A provider profile MUST canonically bind semantic mode, exact model artifact
and digest, available provenance fields, bounds, streaming support, maximum
concurrency, compute profile, latency objective and evidence-window size,
locality, network requirement, retention, accepted sensitivity, tool support,
and structured-output support. A changed profile or model MUST create a new
profile identity and a new exact plan.

### CHAT-005 — independent observation and authority

A local adapter MUST require a caller-owned fresh `ReportCapability` matching
its exact contract/profile and an independently supplied resource/grant
binding. Endpoint, model digest, and provider-profile identity MUST be hashed
authority constraints. Discovery MUST NOT download a model, start a server,
load a model, grant access, or enter ambient facts into compilation.

### CHAT-006 — bounded execution

Execution MUST enforce message, context, output, per-chunk, chunk-count,
concurrency, timeout, response-framing, and retained-evidence bounds. The first
executor proof MAY commit collected response chunks as one finite batch. It
MUST preserve bounded provider framing and terminal evidence and MUST NOT retain
provider conversation state.

### CHAT-007 — distinct terminal causes

Missing or stale provider, empty or overflowing input/context/output, exhausted
concurrency, timeout, cancellation, provider loss, malformed framing,
sensitivity refusal, denied grant, unexpected network, unsupported tools,
unsupported structured output, model mismatch, unsupported profile, and
invalid bounds MUST remain distinct stable reason codes.

### CHAT-008 — redacted evidence

Normative evidence MAY record byte counts, chunk indices, exact provider/model
identities, and terminal reason. It MUST NOT record message/context bytes,
credentials, raw secret material, or generated text. Presentation is a typed
projection of these facts, not a source of provider identity or authority.

### CHAT-009 — honest local adapter

The Ollama-compatible adapter is one optional implementation. Its observer
MUST accept only an explicit binary path, loopback socket address, model name,
and validity interval. Its execution MUST use the exact observed model and
loopback endpoint, request no provider retention, and fail closed on malformed
or mismatched responses. No Ollama-specific semantic node or resolver exists.

### CHAT-010 — executable teaching surface

The checked standalone and composition panels MUST run through the production
executor with the deterministic implementation. The generic inspection
projection MUST expose friendly “quick local model” wording beside exact mode,
implementation, model, locality, retention, bounds, reason, and terminal facts.

## Conformance

[`conformance/c4/quick-local-chat.json`](../conformance/c4/quick-local-chat.json)
owns the positive, negative, boundary, and transition case inventory. Real
models are not compared by prose. Conformance compares admission, exact
binding, bounds, framing, cancellation, redaction, evidence, and terminal
behavior.
