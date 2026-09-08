# Target Hosts and fabrication

Each target family contains the Host, firmware, fabrication, and deployment work needed for that execution environment. Start with [ConduitOS](conduitos/README.md), [RP2040/Pico W](rp2040/README.md), [ESP32](esp32/README.md), or the [repository layout](../docs/repository-layout.md) for the wider source map.

Families use the responsibilities they need; there is no requirement to create an empty directory for every row below.

| Responsibility | Contents |
|---|---|
| `host/` | A separately packaged Host implementation, as in browser. |
| `runtime/` | Lower execution machinery distinct from the Host entrance, as in browser WASM. |
| `offers/` | Separately packaged exact implementation offers, as in std. |
| `fabrication/` | Target descriptors and PROFILE/BUILD/IMAGE contribution; `xtask/` within it owns repository build orchestration when needed. |
| `firmware/` | Firmware projects and their `assets/`; ConduitOS also owns product linker scripts in `firmware/linker/` and Limine configuration in `firmware/boot/`. |
| `deployment/browser/` | Deployment of this target through an admitted browser carrier. These adapters retain target policy; Crèche consumes them. |
| `profiles/` | Reviewed Host configurations and PROFILE examples understood by this family. |
| `tools/` | Target-specific setup, tool installation, credential preparation, and flashing support behind `cargo xtask`. |
| `proof/` | Tightly coupled target appliances and fixtures, not a second general conformance tree. |

Std and ConduitOS retain their Host Cargo package at the family root (`Cargo.toml`, `src/`, and package integration `tests/`). Browser separates its Host server and WASM runtime packages. This is a package boundary choice, not an invitation to add empty Host directories to the other families.

Browser, std, ConduitOS, RP2040, ESP32, AVR, Raspberry Pi and Orange Pi fabrication contributions all live in `fabrication/`. Existing Cargo package names are preserved. For ConduitOS and SBCs, a lightweight descriptor package and build orchestration share that responsibility: `src/` advertises exact targets without loading the builders, while `xtask/` owns image manufacture and proof commands.

RP2040 `network-realization/` remains an independently packaged board-specific network realization. It is not a generic mechanism solely because another board could eventually use it. RP2040 CYW43 bytes and their exact vendored license/provenance live under `firmware/assets/`.

ConduitOS `proof/appliances/<architecture>/` owns each bring-up appliance together with its appliance linker scripts. Product linker and boot configuration live under `firmware/`; the target build script names those exact resources. Repository-wide conformance remains under `proof/` at repository root.

Browser bundles, native bundles, UF2, ESP images, ConduitOS disk images, and SBC images retain their own artifact and deployment contracts. An image build does not establish firmware execution or physical/HIL behavior. Public workflows enter through `conduit`; repository development and hardware proof enter through `cargo xtask`.

Published browser resource URLs retain their existing `targets/<family>/browser-deployment/` package paths. Staging reads source from `deployment/browser/` into those declared resources. Package identities and relative imports are distinct from repository source ownership; the path migration does not change these published resource contracts.
