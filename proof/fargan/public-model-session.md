# Public immutable session composition boundary

`model_compute_owned` adds public no_std immutable model/Source/Plan custody and
load, warm, ready, enqueue, active, finish, cancel, provider-loss and unload
composition over a concrete runtime driver. It retains original Arc identity,
checks the sealed Source/Plan relationship and exact offer/runtime basis, and
requires provider-owned opaque resource/warm/completion/stop receipts. Callers
cannot pass a boolean readiness assertion. Resource admission must cover complete
working and preparation storage; an unknown component must refuse before load.

The additive module passes Clippy against the actual coherent SDK graph from the
80424 BOTH Language/ConduitOS build: Core4ef03007210cad9a,
Plot80b2617ccc91da4f, AI7e238c9a76f0016b. Exact command and Source/artifact hashes
are retained in project `outputs/fargan-public-session-facade`.

This is compile-only API evidence. No concrete public FARGAN provider currently
implements the complete resource admission boundary, and there is no public
runtime or whole-working-memory proof from this module. The earlier manual-root
facade compiled while mixing SDK families; that earlier result was not a coherent
runtime proof and is superseded only at the compile boundary here. Original
trained hosted lifecycle and pressure/resume proofs remain separately scoped.

The preparation receipt is now required and validated before full Plan
verification, literal Source identity hashing, offer cloning, or lifecycle
construction. It must cover those preparation peaks, independently of the later
load working-resource receipt. Source/Plan verification occurs once; subsequent
polls check immutable Arc identity without repeating allocating verification.
Stop and unload also check custody. A foreign basis invalidates the session and
requires explicit cleanup of the retained original model/Source/Plan through
`abandon_original`; it cannot produce a successful foreign stop receipt. These
new paths currently have coherent façade Clippy evidence, not runtime guard-test
or complete preparation-inventory evidence.

The checker identity is the literal `canonical-source:` domain, preserving every
Source byte. The old diagnostic `source-document:` hash is not the checker seal.
A tiny additive identity façade against the unchanged coherent Plot implements
that exact existing checker algorithm for this component gate; production reuse
requires the published Plot helper and a coherent dependent rebuild.
