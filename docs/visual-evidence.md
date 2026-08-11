# Proof-native visual evidence

Visual evidence is a human-inspectable presentation of an already-established
semantic proof. It is not an additional runtime and it does not turn a screenshot
into proof.

`cargo xtask prove browser-host` writes `manifest.json` beneath the deterministic
`target/conduit-evidence/browser-host/` root after the suite terminates. An explicit
`--evidence-root <directory>` may select another root for local or CI consumers.
Evidence declarations always name relative paths beneath that root; absolute paths,
parent traversal, symlinks escaping the root, duplicate identities or paths, more
than 64 outputs, and individual outputs larger than 16 MiB are refused.

The versioned `conduit.evidence-manifest/v1` envelope binds the exact Git commit,
proof and suite identities, completion disposition, declared output metadata, byte
length, and SHA-256 digest. Each output carries a scenario identity and may carry
the proof step, pinned browser/rendering environment, presentation revision, Plan,
active Play, manifestation/renderer identities, and the semantic disposition that
was asserted before capture. Wall-clock time is deliberately absent from evidence
identity.

A successful suite with all required declarations produces `complete`. A failing
or interrupted suite may retain files, but its manifest is
`diagnostic-incomplete`. Missing required evidence also writes an incomplete
manifest and fails evidence validation. Consumers must publish only `complete`
manifests. The manifest format and validation belong to `xtask`; CI may transport
the resulting directory but does not define its meaning.

Issue #821 establishes this contract without declaring any captures. Issue #822
owns the first deterministic Patchbay screenshot declarations and their browser
provenance.
