# Borrowing checked content in default network planning

The default expanded-Plot entrance now delegates to the existing borrowed
connection-limits entrance with an empty override map. It avoids retaining a
second owned CheckedPlot projection while assembling the same Plan. Validation,
source spans, completion, realization provenance and authority/resource checks
remain in that common path. No unchecked or special-purpose network planner
was added.

The CPU64 fixture's fresh network preparation peak falls from 40,898 to 38,528
bytes, removing 39 allocations (1,250 to 1,211). Retained Plan/lowering remains
22,657 bytes; additional kernel preparation remains 8,147 peak and 2,560 retained.
The exact parity regression covers Plan content, completion, source spans,
invalid zero queues and excessive queue requests. All 85 planner unit tests and
nine canonical integration tests pass; the network kernel replay also passes.
Strict planner lint and package formatting pass. The ESP32 native-face-root
release image links with its 13 existing warnings; the public memory ledger
regenerates successfully.

An earlier experiment dropped the decoded Source and current advertisement
before lowering. It left the peak at 40,898 bytes and was removed. Its log is
retained to distinguish a failed hypothesis from the measured clone reduction.

These are CPU allocation observations with fixture authority/radio providers,
not MCU bounds or physical wireless evidence. The native workspace guard is
unchanged. Combined sampler/network admission, radio concurrency and stack high
water remain unproven. No diagnostic was flashed for this change.
