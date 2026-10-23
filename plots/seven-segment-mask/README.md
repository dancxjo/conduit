# Seven-segment Mask

`main.conduit` is an ordinary Face/interaction/Show Mask-role Plot. Both
environmental Body specifications permit it through `wear seven-segment`.
The finite payload and Look facet are authored in `types.conduit`:

```conduit
type SevenSegmentShow = {
    digits: collection SegmentPattern = 4
    decimal_points: collection Boolean = 4
    colon: Boolean
}
type SevenSegmentLook = {
    temperature_unit: SevenSegmentTemperatureUnit
}
```

The temperature-unit alternatives are `fahrenheit` and `celsius`. The selected
Look is an explicit renderer argument. Fahrenheit is the requested application
choice: the captured 19.8°C observation renders as `67.6F`. Changing the Look
does not alter Face content or its quantity's physical meaning.

This Mask-specific Look facet is checked Source data. It has not yet been added
to the universal `LookInfo` carrier's visual/spoken/generative facet inventory.
`SevenSegmentShow` is the finite renderer payload; the existing canonical Show
and MaskShow retain separate Plan/Play, delivery and acknowledgement identities.
Computing a payload does not acknowledge physical output.

## Interpretation

Preparation validates one immutable, existing Face revision with at most four
subjects and sixteen properties. The selected environmental subject supplies
typed `temperature` and `relative-humidity` properties, a `fresh` flag and
`sensor-status` (`available`, `warming` or `unavailable`). Optional
`minute-of-day` and `clock-fresh` properties describe observed clock time; the
Mask does not infer a clock from sensor readings. Preparation retains native
quantities and exact Face correlation. Rendering and selection allocate nothing.

| Selection | Policy |
| --- | --- |
| Temperature | Exact conversion to the Look's unit; one decimal place and `F` or `C`, right aligned. |
| Temperature fitting | Round directly to whole degrees if sign, digits and suffix cannot fit, or the selected wiring cannot express the required decimal point. |
| Humidity | Whole percent with `H`; accept physical values from 0% through 100%. |
| Clock | Four zero-padded digits with the separate colon requested. |
| Rounding | Nearest, ties away from zero. A rounded zero has no minus sign. |
| Overflow | `----`, with an explicit unavailable reason; no clipping or digit loss. |
| Stale or unavailable sensor | `----`; old quantities remain facts in the Face but are not shown as current. |
| Unsupported annunciators | Fit a whole-degree temperature where possible; otherwise `----` with the wiring refusal. |

`Alternation` takes an admitted period and finite lifetime. The caller supplies
elapsed time from the selected clock; the Mask neither creates a timer nor runs
a private scheduler. Temperature and humidity alternate at exact phase
boundaries. Lifetime expiry is explicit.

## Representation and hardware

`SegmentPattern` means seven Boolean segments `a` through `g`. Its checked
`u8 bits` Form maps those fields to bits 0–6; bit 7 is reserved and decoding it
refuses. Native Rust and ECMAScript codecs come from that same checked Form.

The selected `Tm1637Profile` maps the four digit positions and each segment onto
the controller's six display addresses. Decimal points and colon have explicit
optional wire-bit assignments. Reusing an assignment declares a shared control;
contradictory requests refuse. The Mask's `render_for_profile` checks
representability before emitting a payload. No module-specific wiring is
assumed or currently physically qualified.

`../device-protocols/tm1637.conduit` owns the three ordered command packets:
data mode, address plus six mapped display bytes, and display control. Brightness
outside 0–7 refuses. It follows the manufacturer's
[TM1637 V2.4 datasheet](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/datasheet/unit/digi_clock/TM1637.pdf).
Electrical ownership, start/stop edges, ACK observation and bounded timeouts
belong to the selected Host Base. This implementation has not yet installed that
Base or a renderer Back, and does not claim firmware or physical TM1637 output.

Run the focused deterministic proof without CI or firmware builds:

```sh
cargo xtask prove environment mask
```
