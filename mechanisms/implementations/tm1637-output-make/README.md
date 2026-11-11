# Checked TM1637 output construction

This host-side package checks and prepares the shared
[`tm1637-output` Plot](../../../../plots/device-protocols/tm1637-output.conduit).
An application supplies mapped display registers; it need not import an
environmental Body, sampling runtime, temperature policy or private display loop.
The planning code is not linked into a constrained Host's execution image.

`DisplayOutputSource::compile` checks the authored output Plot against the
TM1637 packet types, portable expression/selectors and native write contract.
Its typed fore accepts a `Tm1637Frame` and emits invalid-brightness absence.
The graph encodes packets and selects the native write input; it contains no
GPIO, Host identity, device address, temperature-unit or page-timing policy.
The Mask/Look and selected annunciator profile supply mapped registers upstream.

`PreparedDisplayOutput::prepare` uses ordinary placement/planning and lowering
against supplied current Host offers, selected Bases and independent grants.
It retains the sealed Plan and lowering, including exact typed local fore ports.
This creates no initialized provider, capability possession, membership, installed
Body or permission to start Play. Root must retain the actual implementations
and complete memory/work, native binding and lifecycle admission separately.

The package exposes host-side construction as `conduit-tm1637-output-make`.
The environmental specification-proof executable remains a separate application
consumer. There is no forwarding library at its former Body-local location.

Run `cargo xtask prove environment display-output`. This checks the real authored
graph and ordinary planning, missing-authority and missing-native-offer refusal.
Tests retain actual initialized portable evaluators/selectors and an explicit
CPU pin fixture whose electrical methods panic. Their offers and grants remain
fixture facts. This proof establishes no ESP32 output, live Mask/Look client,
continuous sampling, network transition or recording Host.

The shared package's tests also participate in the ordinary workspace
`cargo xtask check` flow. The environmental proof additionally exercises the
application's display-frame preparation and packet material.
