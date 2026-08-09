# Native Patchbay shell

Issue #555 establishes only the native process boundary for parent #554.

`patchbay-native` uses `winit` with its Wayland backend because it supplies a
real native window and event loop while keeping toolkit types in one adapter
crate. It does not require a browser runtime or introduce a rendering model.
The toolkit remains replaceable because `patchbay-model` has no dependency on
it.

The application model constructs `StdHost` with an explicit minimal
`StdHostComposition`. The resulting ordinary `HostAdvertisement` is the sole
source for the displayed host ID, boot ID, operation capability IDs, and
portable-planner count. UI code cannot add an advertisement or capability.
The initial composition advertises no operation family and one optional
portable planner profile; the native capability required by parent #554 is a
later #559 slice.

Startup produces a bounded Observatory snapshot containing the current
`HostStarted` and `AdvertisementPublished` clue vocabulary. Before event
loop exit, shutdown produces another valid bounded snapshot marking that exact
boot unreachable. The native adapter validates and renders both through the
ordinary Observatory report path. These are current-model reports, not a
Patchbay lifecycle registry. This slice adds no planning, realization, kernel,
compatibility executor, browser worker, membership, or authority path.

`--smoke-exit-after-window` is a finite acceptance aid: the application exits
through the ordinary event-loop path after its first native window cycle, so a
headless Wayland compositor can prove window creation and both lifecycle
reports without timing, forced input, or a second UI path. Unknown or repeated
arguments fail closed.

Canonical checked-face equality from #522 remains the compatibility law when a
later Patchbay request plans semantic work. This shell performs no matching and
therefore neither reintroduces nominal matching nor treats UI labels as
capability identity.
