# #6207: bounded next implementation after Init entry exhaustion

The diagnostic at Source `ffd0479fe`, image SHA-256 `d3a9240a864c7714d0d23d643414a86e295e9a374f7568f2d346410139bd07e4`, is conclusive:

```
CONDUIT_APPLICATION_REFUSAL stage=entry sequence=5 request=Some(Commit) detail=Ok(Fault(WorkExhausted)) entries_interrupts_preemptions=Some((5,64,1))
```

The complete Init Commit exceeds the unchanged 64-RTC-interrupt Application entry budget. It does not identify the internal phase or the completion time required. Actual-bitmap fixture success proves memory fit, not native execution-time fit. No further VM/build/codec experiment or budget increase was performed for this note.

## Smallest concrete next fix: staged typed initialization

Split Init at existing typed ownership boundaries. Root already has `[Option<Initial>; SLOTS]` and each exact `Target`; it does not need to serialize one aggregate Init and teach the service a new resumable JSON parser. Reuse the unchanged Begin/Write/Commit/Read transport, compressed JSON codec, Target/graph/biography schemas, existing constructors, fixed grants and actual allocator.

Add a small closed preparation vocabulary to application-core/data.rs and a hidden builder in models.rs:

1. `PrepareInit { manifest }`: admit the four slots' kinds, target-presence masks and selected targets. The manifest is finite fixed arrays/bitsets; validate identities, slot bounds and that each selected target is declared before allocating the builder. Return Prepared, never Initialized.
2. `PrepareTarget { slot, target, value: Target }`: accept exactly one declared, not-yet-filled target. Decode and validate the existing Target, then move its full graph/plan/body into the existing Patchbay controller constructor. Retain it only in the private builder. No graph omission, digest-only substitute or Root controller execution.
3. `PrepareTutorial { slot, evidence }` / `PrepareTour { slot }`: prepare one existing canonical application per command, preserving the complete biography and current validators/projection. Reject the wrong kind, duplicate slot or undeclared component.
4. `FinishInit`: require exact manifest coverage and valid selections; encode Initialized before moving the completed builder into usable Models. Ordinary model queries, edits, topology and proposals are refused throughout staging. Root does not publish Owner until this acknowledgement and its existing exact graph verification complete.

These are ordinary typed commands on the existing correlated exchange, not retries of Commit or resumption of a preempted guest stack. Any refusal/fault/preemption terminalizes and quarantines the instance, discarding the builder. Duplicate/reordered/missing components or an attempted second initialization cannot produce a usable model. Keep the ordinary one-shot Init only if it cannot bypass the same rules; otherwise remove its production entrance when all callers migrate.

Root's loop has a structural maximum: one manifest, at most SLOTS²=16 targets, at most SLOTS=4 individual application preparations, and one finish: at most 22 commands, each with the already bounded chunk exchange. There is no “continue until ready” loop. The work performed in a target Commit is now one target, rather than decoding/preparing every target and biography together. The retained fixture's whole JSON is 857,351 bytes; each large target is roughly half that payload. This is a precise reduction in aggregate work per entry, **not a claim that the largest target necessarily fits 64 ticks**.

This is the smallest missing dependency worth implementing next. It requires a bounded builder and a few closed commands, not a general parser/state-machine architecture. Concrete files: application-core/{data.rs,models.rs,state.rs}, protected_application.rs/transport.rs initialization caller, and focused existing codec/State/actual-bitmap regressions. Keep metadata 2 MiB, stack 1 MiB, MAX_BLOB 128 KiB and decoded ceiling 1 MiB unchanged. Admission includes builder bookkeeping, retained targets/controllers, biography preparation, incoming and outgoing buffers. Do not retain a second complete Initial graph beside the controller graph.

Tests must cover manifest cardinality/selection, exact graph/plan/body/biography custody, missing/duplicate/out-of-order components, terminal partial initialization, malformed extents and attempts to use incomplete Models, plus the production bitmap allocator over the complete staged exchange and replacements. This tests the implementation rather than hunting another timing explanation.

## Explicit boundary if one target still exhausts

A further split of a single Target's typed decode or canonical validation would require resumable field/validation progress; serde_json Deserialize and the current borrowed canonical validator are not resumable. That is a broader prerequisite, not a free consequence of chunked Write. Do not silently add a general parser, another codec experiment or a larger budget to hide it. The next implementation first isolates real typed units; later native acceptance must establish that those unchanged atomic units are admitted.

## Normal Graph commands remain part of acceptance

The failed Init stopped native Owner::verify_graph, so native Graph timing is unproved. Graph clones and serializes/compresses its entire selected graph inside Commit. Read only transfers already-prepared bytes: more Read chunks do not split reply construction. ProjectGraph also performs full decode, digest validation, projection and reply encoding in one entry. Host tests establish their exact custody and memory fit only.

Full acceptance must admit those commands too. If one exhausts, it needs a separately bounded preparation/encoding slice; Init success alone cannot close the issue. graph_facts::digest currently hashes data::encode(graph), so an incremental encoder must demonstrate exact byte/digest correspondence. Do not silently change custody identities through a different compressed stream.

## Explicit alternative, not a constant bump

A reviewed, measured Application-only **preparation** entry profile could keep one-shot Init with less implementation code. It trades a longer maximum protected turn and a different latency/resource contract for less staging machinery. It must retain a finite independent RTC guard and terminal quarantine, cover maximum/malformed admitted work, and leave other services' shared Draft profile unchanged. The current receipt gives only “more than 64 ticks”, not a justified replacement ceiling or hardware WCET. This is an explicit future tradeoff, not authorization to increase budgets or run another image now.

Current PRs remain blocked until the complete source/image-pinned ordinary lifecycle passes. Host, emulator and physical evidence remain distinct.
