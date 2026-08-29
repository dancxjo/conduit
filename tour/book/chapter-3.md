# Step 6 — Same Face, different implementation

The exact same caller can use either the optimized Host leaf or the reviewed Form Back and produce the same canonical result.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

This side-by-side comparison is a teaching aid, not an author-facing execution preference. Source and checked Form identities agree; expanded Form, Plan, and implementation identities remain honestly different.
