# Try canonical Conduit Forms

The checked-in corpus executes canonical Form source through production
boundaries. These are deterministic acceptance commands, not parser-only
examples: each successful program reaches planning, `conduit-kernel`, host
operation completion, a terminal result, and bounded evidence.

Run commands from the repository root.

## Program 1: text pipeline

```bash
cargo test -p conduit-std-host --test canonical_text_pipeline \
  canonical_program_one_runs_through_the_planner_kernel_and_terminal_evidence
```

The source literal `"Hello, world."` reaches the real `text/upper` and
`presentation/text` offers. Expected presented text:

```text
HELLO, WORLD.
```

The same test target also proves invalid literals and mutated selected
realization/host-operation identities fail before presentation:

```bash
cargo test -p conduit-std-host --test canonical_text_pipeline
```

## Program 2: parameterized reusable form

```bash
cargo test -p conduit-std-host --test canonical_greet
```

The `greet` back expands recursively behind its checked face. Its primitive
`text/literal`, `text/join`, and `presentation/text` leaves plan onto current
host offers and execute through the ordinary kernel. The explicit positional
case presents:

```text
WelcomeTravis
```

The target also proves omitted/default binding and rejects an oversized join or
mutated selected realization before output.

## Program 3: admitted time source

```bash
cargo test -p conduit-std-host --test canonical_clock
```

Positional, named, and lexical-local duration spellings check and expand to the
same semantic identity. Four admitted one-second waits produce:

```text
tick sequence=0
tick sequence=1
tick sequence=2
tick sequence=3
```

No wall-clock sleep is needed in this deterministic test; the host adapter
records the four exact requested durations.

## Program 4: startup, closing flow, and current value

```bash
cargo test -p conduit-std-host --test canonical_count
```

The reusable face distinguishes its startup value, finite normally closing
tick flow, and current observation:

```conduit
form count (
    start: Count = 0
    bump: Tick...| > value: $Count
) {
    cell: state/count(start)
    bump > cell.bump
    cell.value > value
}
```

With `start = 2`, the installed `state/count` and `presentation/count`
operations present exactly:

```text
count value=2
count value=3
count value=4
count value=5
count value=6
```

All five values are admitted before activation; `$Count` does not create an
unbounded history. The negative corpus rejects open/closing temporal mismatch,
non-admissible overflow, and selected-implementation mutation before effects.

## Program 6: unchanged source across std and browser hosts

```bash
cargo xtask prove browser-host --locked
```

The software-gated distributed Signal case loads
`examples/signal-demo.conduit`. That source contains no host, platform,
address, carrier, or WebSocket fact. Planning selects one std source fragment,
one browser/WASM sink fragment, and the exact observed bounded WebSocket link.

Expected software-suite result:

```text
6 passed
2 skipped
```

The two skipped cases require an attached physical Pico and are not part of the
Program 6 claim. The std/browser proof delivers 16 exact receipts, exercises
capacity-one pressure, reaches terminal evidence on both kernels, and leaves no
retained or in-flight value. Its link-break case remains a distinct failure.

## Program 5 boundary

Program 5 is explicitly deferred under #515's permitted stop line. The current
`ConnectionProvider::WebSocket` is a carrier for Conduit sessions; it is not an
authored `net/websocket` operation that intentionally speaks an external
WebSocket protocol.

An honest Program 5 still needs a reviewed semantic duplex checked face, exact
URL/authority/resource admission, std and/or browser host implementation, and a
local deterministic echo server wired through that operation. Reusing the
Conduit session carrier as the authored socket would be a false proof, so no
such substitution is made here.

## Aggregate validation

```bash
cargo xtask check workspace --locked
cargo xtask prove browser-host --locked
```

A green run proves only the software environments and commands above. Thumb or
WASM compilation is not physical device execution, and no command in this page
claims Pico HIL acceptance.
