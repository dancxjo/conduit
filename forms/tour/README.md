# The Conduit Tour

The Tour is canonical journey content plus a portable semantic model consumed
by resident Forms. The former standalone browser application, storage identity,
and product route were retired before v1; browser HTML participates through
`@conduit/browser` rather than owning a private runtime bridge.

Tour does not own body lifecycle truth, a compiler, simulator, scheduler,
or alternate runtime.
If a listing cannot run through a real host, the missing work belongs to that
host or to Conduit's portable semantics.

The [project introduction](../../README.md) explains the motivation. Each lesson should give the human reason for a capability
before asking architectural precision or evidence to carry the explanation:
problem or desire, Conduit idea, executable demonstration, then payoff.

`docs/journeys/tour/` owns the lessons; this Form directory owns the portable
semantic model and canonical source. Keep Form identities and stage
declarations aligned when changing a lesson.

The native ConduitOS Tour consumes the seven-page semantic port, chapter/stage
catalog, Form source identities, and semantic actions. Focus the left pane and
use Page Up, Page Down, Home, or End to read it;
scrolling leaves the laboratory in place. `F3`/`F4` select stages, `F5`/`F6`
select chapters, `F10` runs the current exercise, and `F11` opens the resident
Patchbay over the active forms on the same body.

Linux and Windows packages use their platform desktop presentation
implementations over that same portable application state. A host can choose a
different direct or recursive realization for a form, but it does not get a
private Tour program or progress state.
