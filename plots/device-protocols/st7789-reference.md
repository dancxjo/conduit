# ST7789 single-frame Source protocol

This Source protocol passes public package preparation and pure evaluator
conformance, including its autonomous topology. It contains no pixel codec,
native attachment facts, button handling, or Face rendering proof. Actual kernel
execution and physical display acceptance are separate integration checks.

Concatenate `st7789-types.conduit`, `st7789-control.conduit`, then
`st7789-transition.conduit` in one checked document. Supply native Type imports
for `machine/spi/exchange/{request,result}`,
`machine/digital/access/{request,result}`, and
`machine/clock/at/{request,result}`. Expanding `st7789-command-bank` additionally
requires the standard `bytes/literal` Kind and its native operation factory.
The remaining plots are ordinary checked expressions using bounded Bytes,
byte windows/observations, records, variants, comparisons and integer arithmetic.

`st7789-command-bank` owns the exact twenty command/parameter octets. Its output
passes through `st7789-bank-begin` then `st7789-protocol-initialize`. Initialization
rejects every bank substitution, including changed crop/orientation parameters.
The begin mapper is for the authored literal; it extracts its first twenty bytes.
Call the pure initializer directly to test malformed/substituted Begin values.

The baseline is MADCTL C0, RGB565 high byte first, column 0..239, row 80..319.
Adafruit's Mini PiTFT example additionally rotates the input image 180 degrees in
software. This protocol does not perform that rotation; the packed pixel producer
must supply the intended orientation. Crop and MADCTL must change together if a
subsequent reviewed protocol selects a different orientation.

Wire steps are SWRESET, SLPOUT, COLMOD,55, MADCTL,C0, INVON, NORON, DISPON,
CASET,000000EF, RASET,0050013F, RAMWR. Every step first requests the matching
logical D/C level and waits for that actual observed level before SPI. Successful
SPI input length must equal the output length, even though receive values have
no display meaning. The Source waits 150 ms after SWRESET, 500 ms after SLPOUT,
10 ms after COLMOD's parameter, 10 ms after NORON, and 500 ms after DISPON.
Each delay is based on a fresh completed monotonic clock observation, followed
by a completed deadline request. Early, reversed, refused and overflowing times
refuse instead of retrying.

After RAMWR, D/C is acknowledged high. The protocol requests exactly 225 chunks,
indexed 0..224, each exactly 512 bytes. The next chunk is requested only after
acknowledgment of the preceding SPI transfer. This is 115200 bytes / 57600 RGB565
pixels. Backlight stays off until the entire frame is acknowledged, then a
successful observed-high backlight completion publishes phase 8 completion.

Each action carries its current ordinal; its completion event must echo that
ordinal. Every accepted event increments it. Wrong-kind, stale, refused, short,
wrong-level or out-of-phase results move to phase 9, which requests safe-off.
A valid observed-low safe-off acknowledgment publishes phase 10 refusal retaining
the original finite failure code. A failed safe-off acknowledgment publishes an explicit
safety failure code. An out-of-phase result while awaiting safe-off is terminal unexpected.
A stale event while awaiting safe-off preserves the safe-off requirement.
Only phases 8 and 10 are terminal. Physical safe-off after failed/absent completion
requires cancellation/revocation of the admitted native owner; Source cannot
claim electrical success from a refused operation. Waiting forever for a missing
external chunk likewise needs the containing runtime's cancellation contract.

Compiler/conformance setup needed:

- Check the concatenated document with the three native contract catalogs and
  the standard startup/profile catalogs; compile initialize, action, transition,
  frame and bank-begin with the normal expression machinery.
- Prove the literal maps to exactly 20 octets, fourteen init SPI steps, correct
  D/C changes, exact 150/500/10/10/500ms deadlines, CASET/RASET bounds, then
  225 512-byte pixel exchanges and one final backlight-high action.
- Inject every refusal case at each action family, wrong D/C level, short SPI,
  repeated/out-of-order ordinal, wrong chunk index/length, early/reversed clock,
  clock near U64 maximum, event at phase 8, and safe-off refusal. Require no next
  bus action and explicit safe-off/terminal result as appropriate.
- Preparation/runtime wiring must preserve the ordinal around the raw typed
  Fore requests/results. Logical backlight and command-data actions select
  different already-admitted contacts; requests contain no contact selector.
- Use an existing prepared state/feedback loop, such as the BME280 lifecycle.
  A bank and retained chunk require explicit finite canonical value bounds.
  Successful Source trace proof is distinct from physical transfer/visible image.

Primary references:

- [Adafruit Mini PiTFT guide, 1.3-inch modifications](https://learn.adafruit.com/adafruit-mini-pitft-135x240-color-tft-add-on-for-raspberry-pi/python-setup): 240x240, x_offset 0, y_offset 80, software image rotation 180.
- [Adafruit ST7789 driver](https://github.com/adafruit/Adafruit_CircuitPython_RGB_Display/blob/main/adafruit_rgb_display/st7789.py): command meanings, 16-bit COLMOD 55, inversion, normal/display-on, final MADCTL C0.
- [Adafruit RGB driver](https://github.com/adafruit/Adafruit_CircuitPython_RGB_Display/blob/main/adafruit_rgb_display/rgb.py): RGB565 high-byte-first encoding and software image rotation; separate command/data SPI transactions.
- [Adafruit CircuitPython ST7789 sequence](https://github.com/adafruit/Adafruit_CircuitPython_ST7789/blob/main/adafruit_st7789.py): conservative 150/500/10/10/500 ms waits used here.
- [Adafruit Arduino ST7789 driver](https://github.com/adafruit/Adafruit-ST7735-Library/blob/master/Adafruit_ST7789.cpp): rotation 0 pairs C0 with row offset 80 for 240x240; rotation 2 pairs 00 with offset 0.
- [Sitronix ST7789VW datasheet hosted by Adafruit](https://cdn-learn.adafruit.com/assets/assets/000/092/512/original/ST7789.pdf?1593024947=): §8.6 permits CS pauses at byte boundaries; §9.1.2 reset wait,§9.1.12 sleep-out,§9.1.20–22 address windows and RAMWR; parameters use inclusive end addresses.

The persistent failure field is a finite U64 code; the mapping is declared in
`st7789-types.conduit`. Provider timeout, loss, refusal, unsupported operation,
stale attachment and clock-specific dispositions retain distinct codes. Exact
raw results remain in the input event and are not duplicated into state.

## Autonomous entry

The four pure conformance tests now check every pure entry through
`ProtocolSourcePackage`/`PreparedProtocolEntry`, evaluate the complete frame
trace, and inject provider dispositions, malformed/stale completions, ordinal
exhaustion, and refused safe-off. The expression decoder's depth limit remains
unchanged: the command-bank predicate and phase routing are balanced, and the
transition is a handled-step pipeline. A scalar pending delay keeps clock
arithmetic below that existing limit.

A fifth test prepares the autonomous entry through the production
`protocol_boot::minipitft_source::entry()` helper and checks its public Fores
and exact distinct digital placements. The expanded native gear IDs are
`st7789-autonomous/backlight`, `st7789-autonomous/command-data`,
`st7789-autonomous/spi`, and `st7789-autonomous/clock`.

For `st7789-autonomous`, concatenate these files in order:

1. `st7789-autonomous-imports.conduit`
2. `st7789-types.conduit`
3. `st7789-control.conduit`
4. `st7789-transition.conduit`
5. `st7789-autonomous-helpers.conduit`
6. `st7789-autonomous.conduit`

Prepare exactly these generic specializations (canonical byte envelopes):

- Singleton: `St7789Begin`, 512.
- SeededUntil: `St7789Frame`, 4096.
- FeedbackZip: `St7789State`, 4096; `St7789Event`, 4096.
- Zip: `St7789Context`, 256; `St7789Outcome`, 4096.
- MergeInstance `st7789-contexts`: `St7789Context`, 256.
- MergeInstance `st7789-outcomes`: `St7789Outcome`, 4096.

The entry starts from its authored literal without an external begin input.
Its `chunks` input is `St7789Chunk`; `requested` emits `St7789ChunkAction`.
These external Fores retain the exact type-derived value contracts of their
selected internal endpoints; the generic operation storage envelopes above do
not replace those contracts.
The producer responds only to a surfaced request, using that index; it does
not preload chunks during initialization. `completed` emits the acknowledged
chunk count, and `refused` emits the finite refusal code. Both are U64 flows.
One finite zip retains each ordinal independently of raw Host results, while
one feedback zip joins the resulting event to its exact preceding state.
Logical gears `backlight`, `command-data`, `spi`, and `clock` use the existing
machine Fores; composition must bind the two contacts to distinct retained
native identities. Generic merges, typed zips, state, selectors and expression
operations run in the existing kernel.

Closing the external chunk stream early is a containing-runtime cancellation
condition, not proof that the frame completed. Retain the completed Play and
its native owners while showing a static frame: dropping/cancelling ownership
intentionally revokes the backlight to its safe off level. Transport completion
proves shifted bytes and accepted software observations; it does not prove a
visible physical image or correspondence with the normal Face.
