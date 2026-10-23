# Environmental Face contribution

`contribution` accepts the observation's first-class `Quantity` values and
contributes ordinary universal Face properties. Temperature uses the
`Temperature` value contract; relative humidity uses `Ratio` and an exact
0–100% range. The input units and decimal precision are preserved. A
Fahrenheit Look belongs to the downstream seven-segment Mask.

The caller supplies the checked Plot, Plan and active Play basis. This builder
preserves that basis and supplies one `environment` subject, at most five
properties, and no actions, clock time, controller details or display text.
The Body performs the final Face composition and provenance validation.

Freshness and sensor status are independent. A failed capture may leave a
recent prior observation while the status reports the failure. A stale
observation may coexist with an available sensor. Warming and temperature-only
sensors do not manufacture measurements. Capture timeout, provider failure,
geometry refusal and frame failures retain separate failure labels.

```sh
cargo xtask prove environment face
```

The integration proof composes this contribution into a real Face and passes it
to the existing seven-segment Mask. It checks Fahrenheit manifestation, exact
retention of the original quantities, stale/unavailable output, temperature-only
sensors and invalid-input refusal. The package also checks for
`thumbv6m-none-eabi` without an OS.

Construction allocates ordinary Face objects. This is a bounded contribution
builder, not an installed allocation-sealed MCU Back. Admission of recurring
Face construction, the workload connection and physical display output remain
application integration work.
