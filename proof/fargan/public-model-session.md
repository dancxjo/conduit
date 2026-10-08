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
