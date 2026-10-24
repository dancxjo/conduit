# Seven-segment Mask preparation

`PreparedDisplayMask` consumes an immutable environmental Face and a Look. The
Face retains first-class temperature and relative humidity. The Look chooses
Fahrenheit or Celsius and the fraction annunciator; the hardware profile maps
the finite Show to wired segment, decimal-point and colon bits.

`prepare_show` resolves temperature and humidity once into owned finite material
for that exact Face identity and revision. `render_prepared` selects a page from
an explicit elapsed value and composes the independently checked TM1637 Frame.
No Face borrow survives preparation. Selecting another page neither publishes a
new observation nor advances the Face's freshness clock. At the configured finite
lifetime boundary the presentation becomes unavailable. A newly stale or
unavailable Face resolves to the corresponding unavailable presentation.

These calls allocate no heap after the prepared frame storage exists. Segment
patterns use their checked seven-bit Form; annunciators remain separate until
hardware mapping. This representation does not replace the quantities in Face.

This is inert rendering material, not an initialized Mask role. Its owner must
admit the page clock, invocation and output bounds, retain current Face/provider
custody, replace or revoke obsolete material, and clear manifestation on loss
or cancellation. A typed Frame grants no output authority and performs no GPIO
write. The ordinary selected output Fore and its lifecycle evidence remain
required for physical delivery.
