# Native checked Crèche search proof

Refs #5608 and #5994. The existing canonical live-image proof entrance now has
an independent finite search scenario:

```
cargo +stable xtask make conduitos creche-controls-proof --viewport 640x480 --scenario search
```

The default `layout` scenario preserves the existing invocation. Search uses a
fresh actual guest so its observations fit the existing 256-record and
27-capture bounds. It changes no product state machine, image catalog or limit.
The actual initial catalog must contain more than four offered Plot choices;
the proof records all choices and refuses a smaller inventory rather than
manufacturing additional ones.

Actual keyboard drafts must preserve the complete source Face and acknowledged
Show before Enter. Each Enter must produce one exact checked `creche.search`
interaction and a fresh source publication/acknowledged Show. The source's
Patchbay filter must retain its exact offered action, selection, label and
availability. An unmatched query must remove editable choices while retaining
all selected Plot review text at primary disclosure. Name and naming tradition
remain exact. Explicit Backspace stages an empty replacement query; committing
it must restore the full original inventory and control order. Keyboard focus
traverses all remaining controls forward/backward under the filter and again
after restoration. Every primary page and clamp is observed; sampled original
PNGs retain the existing custody. Exactly three search actions and no selection
changes are allowed.

Harness compilation passes. QEMU enactment and original capture verification
are pending for the clean committed Source. This scenario does not establish
provider-loss fallback, arbitrary query Unicode/IME, physical/human enactment,
combined-head CI or merge. Those remain separately tracked.
