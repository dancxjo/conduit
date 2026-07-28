# Working agreement

- Preserve the separation between semantic contracts, implementations, host
  observations, execution plans, evidence, and presentation.
- `conduit-core` must remain `#![no_std]` and allocator-free. Hosted
  conveniences belong above it.
- Do not add Tongues, Netherwick, Psyched, robot, speech, model-provider, or UI
  concepts to the core.
- Every live cord is bounded. New pressure behavior requires an explicit
  semantic contract and conformance fixtures.
- `.panel` source, resolved plans, run evidence, and Patchbay presentation are
  distinct identities.
- Run focused package tests while developing. Before handoff, run formatting,
  workspace Clippy, workspace tests, and the `thumbv6m-none-eabi`
  `conduit-core` check.
- Keep commits coherent and exclude unrelated concurrent work.
