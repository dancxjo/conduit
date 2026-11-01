# Shared environmental readings

`conduit-environment-measurement` is a `no_std`, allocation-free adapter over Core Quantities. It has no hardware, Source generator, Body admission, wire format, clocks, or signing authority. Both the environmental Face and runtime use the same model. Runtime re-exports it for compatibility.

`EnvironmentalReading` describes a successful reading. Temperature is a temperature Point. Optional humidity and atmospheric pressure use `ReadingField::Available` or `Unsupported`. A bus/calibration/timeout failure must remain a failure at its original protocol boundary; it must never turn into Unsupported or zero. Supported humidity is a Ratio in 0..100%; pressure is a positive Linear pressure-family Quantity.

`temperature_humidity` preserves existing decoded DHT Quantities exactly and marks pressure Unsupported. `centi_celsius_centi_pascal` converts the current portable Bosch driver's fixed coordinates exactly, including Q10 humidity. This is not the older authored BME Source's Q24.8 pressure representation. Neither adapter changes an existing Type, Form, Source identity, signed record, or producer scope.

This model retains three inline Quantity-capable fields, including storage for unsupported variants. Do not install it as an additional permanent ESP32 buffer without measuring the actual profile. It is not an admitted capability advertisement or publication permit. `conduit-environment-face::contribution_reading` accepts it at the existing caller-supplied Face provenance boundary; downstream Mask and Look own presentation units and rounding.
