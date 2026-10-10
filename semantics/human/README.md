# Human interaction quantity boundaries

Absolute scalar controls may carry temperature points, such as `21°C`. Their
reviewed unit, range and granularity remain part of the interaction contract.

The current relative-adjustment family carries `Quantity` payloads. A
temperature adjustment is a difference, so temperature units in this family
return `InvalidContract` until its payload supports an explicit
`TemperatureDifference` value. Use the checked temperature difference operations
for difference conversion and comparison; do not encode a delta as a temperature
point.
