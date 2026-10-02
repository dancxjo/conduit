# Try plots

plots describe portable meaning. The reviewed examples live under
[`plots/`](../plots/README.md), with explicit membership and proof declarations
in [`plots/inventory.toml`](../plots/inventory.toml). A source example can be
checked without having a live implementation of every operation it needs.

Run these commands from the repository root after
[contributor setup](../CONTRIBUTING.md):

```bash
cargo xtask make host std
cargo xtask check plots check
cargo xtask check plots report --output target/plot-conformance.json
```

The first runs Hello on the native host. The second parses and checks the
reviewed inventory. The report describes available proof modes and limitations;
it does not execute all those proofs.

## Read a small program

| plot | What to look for | Current example result |
| --- | --- | --- |
| [Hello](../plots/hello/main.conduit) | A literal flows through `text/upper` to presentation | `HELLO, WORLD.` |
| [Greet](../plots/greet/main.conduit) | A reusable plot with parameters and a checked fore | Explicit positional binding produces `WelcomeTravis` |
| [Clock](../plots/clock/main.conduit) | A standing time source and duration argument | Live between ticks; no completion full stop |
| [Count](../plots/count/main.conduit) | Startup value, closing input flow, and current value | Current count evolves from the configured startup value |
| [Webchat](../plots/webchat/main.conduit) | Bounded chat state and semantic WebSocket operations | A two-page browser proof exercises delivery and disconnect |
| [Signal demo](../plots/signal-demo/main.conduit) | The same source can be planned across different hosts | Native/browser proof produces sixteen receipts |

The first four are a useful reading order. In particular, Count's `$Count`
means a current observation, not an unbounded history:

```conduit
plot count (
    start: Count = 0
    bump: Tick... >> value: $Count
) {
    gear: state/count(start)
    bump >> gear.bump
    gear.value >> value
}
```

## Write semantic ranges

A native Type may omit either end of a numeric range:

```conduit
type Positive = Scalar in 0..
type AtMostOne = Scalar in ..=1
type TemperatureAboveAbsoluteZero = Temperature in -273.15°C..
type Percentage = Scalar in 0..=100
```

`0..` means “no semantic upper bound”; it does not mean “up to the largest
integer in Rust.” Likewise, `..=1` has no semantic lower bound. A bounded range
still names both ends. The fully open spelling `in ..` is rejected because it
adds no meaning beyond the unrefined primitive Type.

Open meaning does not reserve infinite memory. The checked contract separately
records the finite maximum encoding accepted by the selected form.
Fixed-width values therefore cost their fixed extent. A semantically valid
value outside a selected form refuses during lowering or admission.
Any future arbitrary-precision form must admit and charge its actual
encoded extent. Sequence cardinality, stream backlog, and retained state remain
explicitly bounded or governed independently of the element domain.

Variants may carry another semantic Type directly:

```conduit
type LocalModelTerminal =
    produced
    | refused LocalModelRefusal
    | failed LocalModelFailure
    | cancelled
```

The payload remains the named Type; Conduitese does not invent an anonymous
record or make Rust own the relationship. Use `{ ... }` only when the variant
itself owns a record payload.

For an interactive authoring surface, open the native Text Lab:

```bash
cargo xtask prove journey text-lab
```

It begins with an effect-free rehearsal; inspect the selected environment and
use the explicit execution controls when ready. The body Workspace provides
the current browser route through reviewed resident plots.

## Run the declared proofs

```bash
cargo xtask check plots run --deterministic
```

This runs the deterministic checks declared by the inventory. Those checks
range from semantic conformance to complete plan/play execution, so read each
result's proof mode and reason. A parsed plot is not automatically an executed
program. The inventory also records reusable-plot and combined-workload checks.

For the declared browser-safe cases:

```bash
cargo xtask doctor browser
cargo xtask check plots run --browser
```

This mode prepares the browser fixtures and executes eligible inventory cases.
Cases needing devices, permission, credentials, or an unavailable realization
remain explicitly unavailable rather than being counted as passing. The broader
browser suite, including distributed and Webchat scenarios, is:

```bash
cargo xtask prove browser-host --locked
```

The suite owns its builds and browser setup; no separate package build or direct
test-runner command is part of this guide. Its current output is the authority
for test counts. A passing browser case proves the exercised browser behavior;
it does not establish attached-board or physical acceptance.

## Find something to add

Use the inventory and `cargo xtask check catalog matrix` to distinguish authored
plots, installed implementations, and missing realizations. A good contribution
can improve an example, add a meaningful negative check, or complete one
missing implementation. Follow the [contributor guide](../CONTRIBUTING.md) and
[current roadmap](roadmap.md) to keep that change focused.
