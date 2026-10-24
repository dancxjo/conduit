# Committed publication to Face

`PreparedPublicationFace` prepares the exact four publication-channel validators
and projects a validated committed snapshot into an ordinary environmental Face
contribution. It reuses the Face crate's existing semantic result decoder.
Temperature and humidity retain their original first-class quantity bytes.
Decoder failures and capture/provider/geometry failures remain distinct.

Root supplies authenticated publication provenance; the owning Host supplies the
contribution's exact Body Plan/Play basis and freshness policy. This projection
does not derive those facts from data, receipt time or a channel number. It owns
no clock, transport, retry, cache, store or lifecycle. Its ordinary Face allocation
belongs in admitted Host Face construction, outside allocation-sealed Steps.
Failures carry no measurement in this stateless contribution; the existing
`PreparedObservationFace` retains its separate last-valid-value policy.

Run `cargo xtask prove environment face-publication` (also included in `face`).
The CPU tests replay an actual physical canonical observation and compare exact
quantity bytes, check explicit freshness and provenance, reject wrong-channel
bytes, and construct the four decoder failure variants plus provider/geometry
failures from their actual authored payload types. This proves interpretation,
not live Mask delivery. The retained native owner now uses this shared boundary for every authenticated
channel. It saves unavailable-status Faces separately while still requiring
three valid readings for the finite sampling proof. Physical acceptance of this
integration remains pending.

The Root acknowledgement transcript version 2 binds committed and inspected
monotonic Boot times along with the exact publication sequence, channel, length
and digest. Future commit times refuse verification. The finite owner admits a
maximum age of twice the checked Source cadence and adds its entire measured
inspection round trip to the signed age as a conservative transport bound.
Reversed time and arithmetic overflow cannot become fresh. Receipt verification
still binds the exact current command and the already admitted Boot key.
These are publication commit times, not claims about precise sensor acquisition
time or a global wall clock. Refs #6347.
