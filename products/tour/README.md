# The Conduit Tour

The repository-development Tour teaches Conduit by running and changing
ordinary forms. A Markdown fence marked `conduit run` is sent unchanged to the browser host's Rust/WASM
parser, checker, planner, lowering layer, and production kernel.

From a repository checkout, open it with:

```sh
cargo xtask demo tour
```

Tour owns the bounded `conduit.application/tour-reading-state` storage identity
and refuses malformed or over-capacity state. It is not a public Pages product;
the retired Book and Tour routes and the Book saved-state dialect are not part
of the pre-v1 product contract.

Tour does not own body lifecycle truth, a compiler, simulator, scheduler,
or alternate runtime.
If a listing cannot run through a real host, the missing work belongs to that
host or to Conduit's portable semantics.

The [project introduction](../../README.md) explains the motivation. Each lesson should give the human reason for a capability
before asking architectural precision or evidence to carry the explanation:
problem or desire, Conduit idea, executable demonstration, then payoff.

`content/` owns the lessons, `model/` the portable application model, and
`browser/` the reader and lesson interactions. Executable fences and front
matter are consumed by the application; keep their canonical form identities
and stage declarations aligned when changing a lesson.

The native ConduitOS Tour consumes the same seven-page application port,
chapter/stage catalog, form source identities, and semantic actions as the
browser Tour. It uses the bounded ConduitOS compositor instead of the browser
DOM. Focus the left pane and use Page Up, Page Down, Home, or End to read it;
scrolling leaves the laboratory in place. `F3`/`F4` select stages, `F5`/`F6`
select chapters, `F10` runs the current exercise, and `F11` opens the resident
Patchbay over the active forms on the same body.

Linux and Windows packages use their platform desktop presentation
implementations over that same portable application state. A host can choose a
different direct or recursive realization for a form, but it does not get a
private Tour program or progress state.
