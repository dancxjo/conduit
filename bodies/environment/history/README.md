# Environmental history Face and graphical Masks

Preparation slice [#6464](https://github.com/dancxjo/conduit/issues/6464),
continuing [#6461](https://github.com/dancxjo/conduit/issues/6461), using
[#6295](https://github.com/dancxjo/conduit/issues/6295)'s existing recorder.
The MCU image and local sensing/display remain owned by
[#6457](https://github.com/dancxjo/conduit/pull/6457).

`PreparedHistoryFace` wraps the existing `PreparedRecordingHistory` workspace.
It validates ordered chunks for one independently supplied producer
Host/Boot/Plan/Play and projects the latest loaded chunk into an ordinary Face
fragment. At most 64 samples are visible. Earlier recovered samples are counted
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
producer-Boot time coordinates and independent Celsius/percent axes. The native adapter joins consecutive available observations with bounded
polylines; failures, missing sequences and producer-Boot boundaries break the
line, and isolated samples remain dots. The browser adapter retains its existing
observation dots. Neither fabricates samples across failures or gaps. The
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
Mask Plan, production kernel and compositor:

```sh
cargo +stable xtask prove environment history-native
```

It supplies a synthetic recording Face to the pending renderer, rasterizes the
history scene into retained CPU pixels and completes the original renderer only
with compositor-owned scanout acknowledgement. Its Host offers and display are
fixtures. It does not install a native Host, join the MCU Body or establish
physical Pi/HDMI scanout.

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

`HistoryFaceWindow::prepare` selects an explicit tail before Face construction,
without discarding recorded samples or advancing recovery. Samples outside the
view remain counted. This sample budget does not replace admission of the full
composed Face against the selected Host byte limits.

`RetainedEvidenceGraph` is a separate read-only projection of at most16 original
canonical observed records supplied by an independently verified retained-evidence
exporter. It shares chart geometry and finite native graphics, but creates no
Face contribution, current Play, selected recording State or commit receipt. Its
native page explicitly says `RETAINED HISTORY / SIGNED EVIDENCE`; producer Boot
milliseconds remain distinct from wall time. The integrating Host verifies
signatures and binds the private input before this inert representation check.
