---
page: faces-and-backs
route: faces-backs-and-implementation
companion: recursive-form
stage: canonical-form:same-morse-caller|compare
---
# faces, backs, and implementation

When one gear calls another, it depends on the called gear's **face**: meaning plus typed port contract. It does not need the callee's implementation details.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

In Patchbay, open the reviewed back for `same-morse-caller/morse`. The caller stays unchanged while you inspect a checked internal form made of smaller gears. Close the back and you are back at the same face.

That distinction matters: the caller composes against stable semantics, while hosts can realize the inside differently. A rich host might provide `text/morse` directly. A smaller host can keep opening reviewed backs until remaining leaves match machinery it can actually provide.

So the next question is unavoidable: who can realize those leaves right now?
