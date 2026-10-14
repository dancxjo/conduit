# Names at Face construction

Every authored `face/*` Gear declares the meaningful name of its data when it
is constructed. The first startup parameter is required bounded `Text` named
`label`; it has no implicit default. A Gear variable such as `show` identifies
an operation in the Plot and does not name the information shown to a person.

```conduit
plot greeting {
    show: face/text("Greeting")
    "Hello, world." >> show
}.
```

Named arguments work too, alongside the Gear's existing configuration:

```conduit
plot greeting {
    show: face/text(label = "Greeting", maximum-values = 3)
    "Hello, world." >> show
}.
```

This contract covers the whole authored Face family: simple typed sinks,
structured information, application views, scene and component construction,
renderers, forwarding, interactions, and spoken Mask operations. Existing
parameters such as icon selection, layout role, title, and output bounds remain
separate from the required data name.

Names must contain non-whitespace content, contain no control characters, and
fit within 128 UTF-8 bytes. Icon, frame, and badge names retain their existing
tighter 64-byte semantic storage bound. Source checking requires the argument; canonical
expansion checks its typed value and rule. Installed native and browser
preparation check the same name independently before retaining runtime values.

Names remain beside exact typed values. Naming a Quantity does not change its
unit, value kind, canonical bytes, or structural envelope, and does not turn it
into Text. Plans retain the construction name; realized structured value Signs
retain it, and their Face projection uses it for the root subject's human name.
Native text output and browser effects retain the name with each value. Simple
interfaces can show these named values in their existing ordered slots or list;
they need no application-specific schema to understand the data name.

Authored names use `face/*`, the semantic crate is `conduit-face`, and its
carrier is `Face`. The Body projection wrapper is `BodyFace`. The retired
`presentation/*` Gear names are not aliases. See the
[vocabulary migration](face-vocabulary-migration.md) for the classified rename
and the handling of historical evidence.
