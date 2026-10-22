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

The original `contribution` builder allocates ordinary Face objects and remains
available for dynamic Body composition. The prepared publication path below
serves recurring MCU observations. Installed Host/Body construction and physical
display output remain separate integration work.

`PreparedObservationFace` reserves the recurring grammar and exact quantity
leaves before Play. It owns the current immutable universal Face revision;
measurements are absent while warming. A committed result updates the typed
quantities or the distinct failure status, retaining prior valid values and
sample time on failure. Inspection advances freshness only when it changes.
The sample ceiling, property capacities, text capacities and stale interval are
explicit preparation inputs. Publication, expiry and validation allocate nothing
in the tested environmental profile. Refused input preserves the prior revision.

The universal Face identity remains the ordinary canonical identity, including
its exact Source/Plan/Play basis. Contract identity bytes stream directly into
its hash; `Face::reseal_prepared` validates changed content and writes the new
identity into constructor-reserved storage. There is no sensor-specific Face
identity or renderer-owned observation cache.
