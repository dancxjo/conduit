# Try canonical Conduit Forms

The checked-in corpus executes canonical Form source through production
boundaries. These are deterministic acceptance commands, not parser-only
examples: each successful program reaches planning, `conduit-kernel`, host
operation completion, a terminal result, and bounded Sign.

Run commands from the repository root.

The exact canonical sources are checked in under `examples/*.conduit` and
`fixtures/forms/*.conduit`. The acceptance tests below load those files directly,
so the documented programs cannot drift into test-only string literals.

## Program 1: text pipeline

```bash
cargo test -p conduit-std-host --test canonical_text_pipeline \
  canonical_program_one_runs_through_the_planner_kernel_and_terminal_sign
```

The source in `examples/hello.conduit` sends the literal `"Hello, world."` to
the real `text/upper` and
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

The `greet` back in `examples/greet.conduit` expands recursively behind its
checked face. Its primitive
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

The positional specimen is `examples/clock.conduit`; named and lexical-local
duration spellings remain explicit semantic-equivalence vectors in the same
test. All three check and expand to the same semantic identity. Four admitted
one-second waits produce:

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

The reusable face in `examples/count.conduit` distinguishes its startup value,
finite normally closing tick flow, and current observation:

```conduit
form count (
    start: Count = 0
    bump: Tick...| > value: $Count
) {
    gear: state/count(start)
    bump > gear.bump
    gear.value > value
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

All five values are admitted before Play start; `$Count` does not create an
unbounded history. The negative corpus rejects open/closing temporal mismatch,
non-admissible overflow, and selected-implementation mutation before effects.

## Program 6: unchanged source across std and browser hosts

```bash
cargo xtask prove browser-host --locked
```

The software-gated distributed Signal case loads
`examples/signal-demo.conduit`. That source contains no host, platform,
address, Line, or WebSocket fact. Planning selects one std source fragment,
one browser/WASM sink fragment, and the exact observed bounded WebSocket link.

Expected software-suite result:

```text
6 passed
2 skipped
```

The two skipped cases require an attached physical Pico and are not part of the
Program 6 claim. The std/browser proof delivers 16 exact receipts, exercises
capacity-one pressure, reaches terminal Sign on both kernels, and leaves no
retained or in-flight value. Its link-break case remains a distinct failure.

## Program 5 boundary

Program 5 is the bounded local webchat in `examples/webchat.conduit`. Its source
intentionally names the semantic `net/websocket` and `net/websocket/listen`
operations. The checked client face uses `WebSocketMessage`, not a generic byte
stream, so #522 compatibility remains exact face equality without relying on
the operation name to carry protocol meaning.

The browser plan combines portable `chat/state`, `presentation/tee`,
`presentation/renderer`, `presentation/interaction`, and `chat/submit` with
the native WebSocket capability. Authored labels, action availability, input
type and byte bound, connection status, and the sixteen-item history bound are
Presentation truth. JavaScript is only a generic semantic renderer and human
input adapter; it does not own chat policy. The std plan selects the separately
installed bounded listener. Both execute through fixed kernels. Two Chromium
pages prove click and Enter gestures, A then B delivery, one-page disconnect,
continued delivery to the remaining page, and a source-only label oracle.
Presentation, Manifestation, interaction, Form, checked, expanded, Plan,
fragment, Play, placement, and host-operation identities are retained without
recording message content.

This is mechanically distinct from `ConnectionBase::WebSocket`, which
transports Conduit sessions between Hosts. No Line, link binding, or session
frame appears in the authored external-WebSocket plan.

Focused proof:

```bash
cargo build -p conduit-browser-runtime --target wasm32-unknown-unknown --release
cargo build -p conduit-std-host --bin webchat-server
cargo xtask prove browser-host
```

## Aggregate validation

```bash
cargo xtask check workspace --locked
cargo xtask prove browser-host --locked
```

A green run proves only the software environments and commands above. Thumb or
WASM compilation is not physical device execution, and no command in this page
claims Pico HIL acceptance.
