# ESP32 cooperative pulse capture

The native provider owns one selected GPIO and implements the existing bounded
pulse geometry. It never recognizes a DHT11 or duplicates the protocol Source.
It drives only low, releases to an input with a 3.3 V internal pull-up, and packs
only complete sampled octets. Partial octets remain private. Timeout, complete
capture and refused geometry remain distinct.

With the pinned ESP32-WROOM SDK installed and activated:

```sh
cargo xtask prove environment pulse-esp32
```

This cross-checks the provider and generic Host Call trait for ESP32. It does not
establish a physical capture, an initialized Host offer, admission or a Body.
The active attachment request is GPIO27/D27; the provider does not choose a pin.

The implementation uses one cooperative poll: hold-low is a bounded busy delay
and the capture phase excludes interrupts for at most its selected deadline,
with a 10 ms capture ceiling and 50 ms hold ceiling. Native installation must
admit that electrical ownership, CPU work and interrupt exclusion before making
an available offer. It must also account for native GPIO/timer overhead and its
interrupt scheduling contract. This is not an asynchronous hardware-offload
claim. Concurrent radio operation and physical timing still require measurement.
Every returned capture releases the output; dropping an unpolled future has no
effects, and no await yields while the pin is driven. Recurring capture allocates
no storage. Source retains hold/skip/sample/count semantics and all sensor decode,
range and checksum rules.
