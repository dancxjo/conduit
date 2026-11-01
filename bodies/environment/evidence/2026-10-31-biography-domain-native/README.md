# Biography physical-dispatch adapter: native build only

Native code at `a10cdc6074` passed the supported entrance:

```
cargo +stable xtask prove environment local-appliance-image OWNER_PUBLIC_KEY
```

Construction used the pinned `esp-conduit-1.91.1` toolchain and separately
retained fresh birth/owner inputs. The native build completed in 10m11s; the
linked packaging check confirms the indexed task planner and absence of runtime
Source parsing/checking/expansion. The check retained 168 runtime packages,
including 26 reviewed Conduit packages. Existing compiler warnings were emitted;
this is not strict Clippy qualification.

The packaged application is 3,173,664 bytes against the 3,371,008-byte app
partition (197,344 bytes of margin). The linked stack reserve is 37,852 bytes
against the existing 36,864-byte floor (988 bytes of margin). The ELF file's
7,561,128-byte extent includes non-loadable content and is not app flash usage.
Exact hashes are in `receipt.json`; image bytes remain private.

This proves compilation, linking and static packaging only. The image has fresh
construction inputs and is **not an image for Blue's retained identity**. It was
not flashed. Stack reserve is not measured stack high-water; no runtime peak
heap, flash/codec WCET, owner interruption or sensor/display continuity was
measured. The physical queue adapter has no live management caller.

The later live-owner correction from `153b54518d` (integrated as `53183b11ed`)
is not included in this artifact. Its actual Store/owner handoff, qualified quiet
read/codec phases, static queue integration and physical continuation remain
separate gates. Do not reuse this artifact as their native or hardware proof.
