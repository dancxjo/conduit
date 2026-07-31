# Reference distribution profiles

These sealed JSON documents describe Conduit's safe reference provider
inventory:

- `reference-hosted.json`;
- `reference-browser.json`;
- `reference-constrained.json`.

Each profile pins the genesis profile, control recorder, provider-enablement
class and operation, finite limits, and every provider's exact descriptor,
artifact status, availability, and generic risk traits. A provider with any
dangerous trait is absent, disabled, or unsupported by default.

The documents are package observations, not execution plans or authority
grants. They do not install, fetch, enable, execute, or prevent an external
package manager from installing software. Supplying one in an explicit compile
input lets `conduct --check` and `conduct --explain` fail with the exact
unavailable provider; it does not relax implementation, host, realm, passport,
artifact, or authority requirements.

Deliberately enabling a dangerous provider requires the bounded,
artifact-pinned, independently approved operation in
[`spec/044-safe-genesis-and-distribution.md`](../spec/044-safe-genesis-and-distribution.md).
Provider enablement grants no effects.
