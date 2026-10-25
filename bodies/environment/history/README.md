# Environmental history Face and graphical Masks

Preparation slice [#6464](https://github.com/dancxjo/conduit/issues/6464),
continuing [#6461](https://github.com/dancxjo/conduit/issues/6461), using
[#6295](https://github.com/dancxjo/conduit/issues/6295)'s existing recorder.
The MCU image and local sensing/display remain owned by
[#6457](https://github.com/dancxjo/conduit/pull/6457).

`PreparedHistoryFace` wraps the existing `PreparedRecordingHistory` workspace.
It validates ordered chunks for one independently supplied producer
Host/Boot/Plan/Play and projects the latest loaded chunk into an ordinary Face
fragment. At most 64 samples are visible. `HistoryFaceWindow::prepare` explicitly
selects a smaller tail before constructing the Face when required by a Host
profile. This does not discard recorded samples or advance the reader: recovered
counts still include the full chunk, and omitted samples are reported. The
sample budget is not a substitute for checking the composed Face against the
selected Host's actual byte limits. Earlier recovered samples are counted
as outside the view; history before the first recovered sample remains unknown.
The history schema requires paired temperature/humidity for available samples.
Sequence gaps and acquisition coordinates in the producer's Boot clock remain
explicit. Failed samples have a failure disposition and no copied prior values.
First-class temperature and relative-humidity quantities retain their exact
semantic contracts and source precision.

The inspecting Plot's contribution basis and the producer's context remain
distinct. A Body may compose this fragment beside the existing `environment`
current-reading contribution. The seven-segment Mask continues to select only
that current-reading subject. Graphical Masks select the exact history collection
subject after Body composition, including any contribution namespace.

Run the focused content/projection and Mask proof:

```sh
cargo +stable xtask prove environment history
```

The shared graph preparation selects at most the latest 16 samples, counting
all older samples outside the view. Both graphical adapters use the same actual
producer-Boot time coordinates and independent Celsius/percent axes. They show
observation dots without interpolation across failed or missing samples. The
browser also supplies a semantic data table and an inspectable source context;
the native material uses the existing finite `GraphicsScene` grammar with exact
Face identity/revision retained beside its payload. These are inert preparation
adapters, not yet installed checked Mask Plays or acknowledged Shows.

```sh
cargo +stable xtask prove environment history-target
cargo +stable xtask prove environment history-preview
cargo +stable xtask prove environment history-browser
```

The target command checks the library for `wasm32-unknown-unknown`; it does not
execute a browser. The preview writes an explicitly synthetic twelve-sample
fixture under `target/environment-history-preview`, including its source chunk,
sealed Face, browser HTML and native scene bytes. The browser command runs pinned
Chromium with one worker and zero retries, checking the rendered fixture, failure
absence, explicit gaps, source inspection and mobile fit. Its screenshots prove
that fixture's HTML rendering, not a joined browser Host or physical readings.
Native scene bytes are not native rendered pixels or Pi scanout evidence.

The native CPU integration proof uses the existing checked native graphical
Mask Plan, production kernel and compositor rather than another Mask runner:

```sh
cargo +stable xtask prove environment history-native
```

It supplies a synthetic recording Face to the pending renderer, rasterizes the
history scene into retained CPU pixels and completes the original renderer only
with compositor-owned scanout acknowledgement. Its Host offers and display are
fixtures. It does not install a native Host, join the MCU Body or establish
physical Pi/HDMI scanout.

The reviewed `environment-history-mask` root binds exactly the
`environment-history` collection and its required producer/clock/count fields.
The browser owner Mask prepares graph coordinates from that same checked Face;
its DOM component adds a responsive plot and exact-value table while retaining
all original generic Face content. Numeric observation values and revisions
cross the JS boundary as strings; only finite drawing coordinates are numbers.
Refused history retains the generic Face rather than drawing guessed points.

```sh
cargo +stable xtask prove environment history-owner-browser
cargo +stable xtask prove environment history-owner-target
cargo +stable xtask prove environment history-cross-mask
```

This proof exports the actual prepared browser Mask view, renders it in pinned
Chromium at desktop/mobile widths, and separately verifies the real kernel's
matching-acknowledgement gate. A DOM screenshot plus a CPU acknowledgement test
is not a joined-Host transport receipt. The native offered-Host preparation
helper admits the same reviewed source; native presentation selection and the
Body-owner recipe selection route remain separate integration work. The Look
selector files currently owned by #6469 are not modified here.

The cross-Mask entrance feeds the exact exported, sealed browser fixture Face
to the configured native Mask, preserving its Body basis and revision without
creating another Body on the native side. Both ordinary kernels and actual DOM/
CPU pixel rendering participate; the carrier/Host inputs remain fixtures. This
is neither a live MCU Line nor physical display or fresh-process recorder proof.
The target entrance checks the production browser runtime library for wasm32.

LookInfo v3 carries an optional quantity facet, independent of palette/density.
The shared presentation formatter converts the original temperature Point to
`°C`, `K`, or `°F` through exact Core laws, then rounds once to three display
places (nearest, ties away from zero). Graph coordinates retain the original
exact Celsius projection, so an affine unit change does not move the dots.
Both graphical preparation adapters accept that Look, and the configured
browser/native renderer consumes its selected quantity facet. The browser table
separates rounded selected-unit values from exact source temperatures.

```sh
cargo +stable xtask prove environment history-look
```

Celsius remains the history Mask's authored default when the quantity facet is
absent. Interactive per-Mask selection, retained typed user defaults/overrides,
and seven-segment Kelvin presentation still require integration for #6461.
This contract/formatter preparation does not claim those controls or storage.

This is hosted preparation and deterministic content/Mask proof. Fragment
construction allocates; complete memory/work admission remains the integrating
Host's responsibility. Neither decoding nor Face construction establishes an
authorized storage read, durable write, kernel sink commit, Body membership,
a current producer, a live Line or an installed recorder. Root must connect
only independently authorized recording generations after their required
commit boundary. Reopening raw chunk JSON alone cannot make that claim.

The remaining delivery requires the ordinary recorder publication/catalog seed
and restart path (#6354, #6359, #6360, #6358), browser/native Host installation,
ordinary invitations and planned transport to the existing MCU Body, and actual
browser/native enactment. Native Pi/HDMI and MCU deployment keep their physical
proof boundaries (#6325, #6376, #6457). This package creates no alternate storage,
scheduler, sensor acquisition, Body birth or transport service.

## Commit-bound history

`PreparedCommittedHistoryFace` retains a borrowed, immutable selection of at most
64 publication observers obtained from the actual `RecordingSnapshotBack`s.
Resource semantic identity plus version must be unique; changing reference
extent or other metadata cannot admit that generation twice. The selected
observer cannot be replaced during a read. Before publication output commit,
the exact same valid JSON still refuses as uncommitted. After commit, the
observer matches the digest of its original bounded input; equal counters or a
returned reference cannot substitute other bytes. Existing producer-context,
clock, replay, gap and capacity checks then apply before advancing the view.

The Face distinguishes no publication evidence from `kernel-publication-commit`
and retains the last exact first-class resource reference. A digest match is
process-local kernel evidence, not authority or a portable receipt. A fresh
process must use the separately admitted recovery/catalog/seed path; it cannot
reconstruct these observers from JSON or a reference.

```sh
cargo +stable xtask prove environment history-commit
```

This entrance runs the existing runtime recording tests and strict library/
recording-target lint, then all history tests and strict all-target history lint.
The Linux integration services the original production-kernel request using the
existing immutable file primitive. Only after the exact request completes and
its Back output commits can the history Face advance. It reopens the real file
under a different Boot's binding and feeds identical bytes to another view using
that same retained observer. Occupied-generation failure preserves the prior
record file and Face; stale authority denies before file effects. The source,
placement, offer and independent authority inputs remain explicit CPU fixtures.
This establishes actual Linux file effects and kernel correlation, not an
installed native Host, live disk-capacity admission, fresh-process recovery,
Body membership, MCU transport, native pixels or physical scanout.

The cross-Mask proof also recovers a full 64-sample chunk, projects an explicit
eight-sample tail and admits its exact exported Face through both configured
Masks. All 56 samples outside that view remain counted; the recording is intact.

## Browser owner receiver Look input

`cargo +stable xtask prove environment history-browser-look-receiver` exercises
the production owner-Mask ABI and its JavaScript byte bridge. An independent
canonical Look uses a separate bounded 64 KiB input arena; the owner Face frame
retains its 60 KiB ceiling. The receiver reproduces the complete checked history
Plan using the actual admitted carrier limits. Wrong or missing startup input,
foreign Roots, altered queues and oversized frames refuse while retaining the
previous acknowledged Show. An unchanged response requires the same independent
Look custody. The tests use CPU lifecycle and carrier fixtures; this entrance
does not establish live owner selection, browser SDK preference retention,
saved defaults, DOM enactment or an installed environmental recorder.
