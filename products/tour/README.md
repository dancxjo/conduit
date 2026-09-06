# The Conduit Tour

Tour presents Markdown-guided pages and
ordinary Conduit Forms. A
fence marked `conduit run` is sent unchanged to the browser Host's Rust/WASM
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

The root README's **Why Conduit exists** section is Tour's motivational
source of truth. Each lesson should give the human reason for a capability
before asking architectural precision or evidence to carry the explanation:
problem or desire, Conduit idea, executable demonstration, then payoff.
