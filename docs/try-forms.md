# Try forms

forms describe portable meaning. The reviewed examples live under
[`forms/`](../forms/README.md), with explicit membership and proof declarations
in [`forms/inventory.toml`](../forms/inventory.toml). A source example can be
checked without having a live implementation of every operation it needs.

Run these commands from the repository root after
[contributor setup](../CONTRIBUTING.md):

```bash
cargo xtask fabricate host std
cargo xtask check forms check
cargo xtask check forms report --output target/form-conformance.json
```

The first runs Hello on the native host. The second parses and checks the
reviewed inventory. The report describes available proof modes and limitations;
it does not execute all those proofs.

## Read a small program

| form | What to look for | Current example result |
| --- | --- | --- |
| [Hello](../forms/hello/main.conduit) | A literal flows through `text/upper` to presentation | `HELLO, WORLD.` |
| [Greet](../forms/greet/main.conduit) | A reusable form with parameters and a checked front | Explicit positional binding produces `WelcomeTravis` |
| [Clock](../forms/clock/main.conduit) | A finite time source and duration arguments | Four admitted ticks |
| [Count](../forms/count/main.conduit) | Startup value, closing input flow, and current value | Values 2 through 6 |
| [Webchat](../forms/webchat/main.conduit) | Bounded chat state and semantic WebSocket operations | A two-page browser proof exercises delivery and disconnect |
| [Signal demo](../forms/signal-demo/main.conduit) | The same source can be planned across different hosts | Native/browser proof produces sixteen receipts |

The first four are a useful reading order. In particular, Count's `$Count`
means a current observation, not an unbounded history:

```conduit
form count (
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
records the finite maximum encoding accepted by the current representation.
Fixed-width values therefore cost their fixed extent. A semantically valid
value outside a selected representation refuses during lowering or admission.
Any future arbitrary-precision representation must admit and charge its actual
encoded extent. Sequence cardinality, stream backlog, and retained state remain
explicitly bounded or governed independently of the element domain.

For an interactive authoring surface, open the native Text Lab:

```bash
cargo xtask prove journey text-lab
```

It begins with an effect-free rehearsal; inspect the selected environment and
use the explicit execution controls when ready. The body Workspace provides
the current browser route through reviewed resident forms.

## Run the declared proofs

```bash
cargo xtask check forms run --deterministic
```

This runs the deterministic checks declared by the inventory. Those checks
range from semantic conformance to complete plan/play execution, so read each
result's proof mode and reason. A parsed form is not automatically an executed
program. The inventory also records reusable-form and combined-workload checks.

For the declared browser-safe cases:

```bash
cargo xtask doctor browser
cargo xtask check forms run --browser
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
forms, installed implementations, and missing realizations. A good contribution
can improve an example, add a meaningful negative check, or complete one
missing implementation. Follow the [contributor guide](../CONTRIBUTING.md) and
[current roadmap](roadmap.md) to keep that change focused.
