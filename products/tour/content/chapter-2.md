---
page: faces-and-backs
route: faces-backs-and-implementation
companion: recursive-form
stage: canonical-form:same-morse-caller|compare
---
# Faces, Backs, and implementation

When one Gear calls another, it depends on the called Gear's **Face**: meaning plus typed Port contract. It does not need the callee's implementation details.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

In Patchbay, open the reviewed Back for `same-morse-caller/morse`. The caller stays unchanged while you inspect a checked internal Form made of smaller Gears. Close the Back and you are back at the same Face.

That distinction matters: the caller composes against stable semantics, while Hosts can realize the inside differently. A rich Host might provide `text/morse` directly. A smaller Host can keep opening reviewed Backs until remaining leaves match machinery it can actually provide.

So the next question is unavoidable: who can realize those leaves right now?
