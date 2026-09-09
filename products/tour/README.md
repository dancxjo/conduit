# The Conduit Tour

[Open the Tour](https://dancxjo.github.io/conduit/tour/) to learn Conduit by
running and changing ordinary Forms. A Markdown fence marked `conduit run` is sent unchanged to the browser Host's Rust/WASM
parser, checker, planner, lowering layer, and production kernel.

From a repository checkout, open it with:

```sh
cargo xtask demo tour
```

The current product route is `/tour/` (`/conduit/tour/` on Pages). The old
`/book/` Pages route redirects to Tour, preserving its query and fragment.
Tour retains the historical `conduit.application/book-reading-state` storage
compatibility identity so existing drafts remain accessible. Its bounded
reader accepts the historical reading-state schema and writes the Tour schema;
it refuses malformed or over-capacity state. This is saved-state compatibility,
not a second executable Book product.

Tour does not own Body lifecycle truth, a compiler, simulator, scheduler,
or alternate runtime.
If a listing cannot run through a real Host, the missing work belongs to that
Host or to Conduit's portable semantics.

The [project introduction](../../README.md) explains the motivation. Each lesson should give the human reason for a capability
before asking architectural precision or evidence to carry the explanation:
problem or desire, Conduit idea, executable demonstration, then payoff.

`content/` owns the lessons, `model/` the portable application model, and
`browser/` the reader and lesson interactions. Executable fences and front
matter are consumed by the application; keep their canonical Form identities
and stage declarations aligned when changing a lesson.

The native ConduitOS Tour shows the first chapter’s prose beside its canonical
Form. Focus the left pane and use Page Up, Page Down, Home, or End to read it;
scrolling leaves the laboratory in place.
