# The Conduit Tour

The tour is Markdown plus presentation assets around ordinary Conduit Forms. A
fence marked `conduit run` is sent unchanged to the browser Host's Rust/WASM
parser, checker, planner, lowering layer, and production kernel.

From a repository checkout, open it with:

```sh
cargo xtask demo book
```

The tour does not own a compiler, simulator, scheduler, or alternate runtime.
If a listing cannot run through a real Host, the missing work belongs to that
Host or to Conduit's portable semantics.

The root README's **Why Conduit exists** section is the Tour's motivational
source of truth. Each lesson should give the human reason for a capability
before asking architectural precision or evidence to carry the explanation:
problem or desire, Conduit idea, executable demonstration, then payoff.
