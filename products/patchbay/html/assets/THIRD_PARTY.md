# Browser presentation assets

The browser Patchbay vendors four presentation-only assets recovered byte-for-byte from
`origin/archive/pre-reboot-2026-08-04` at source commit
`5ce3588b5711aa58d52c73bd1bd9e02b3fc0032d`:

| Asset | SHA-256 | License/provenance |
|---|---|---|
| `react.min.js` | `d949f1c3687aedadcedac85261865f29b17cd273997e7f6b2bfc53b2f9d4c4dd` | React production UMD; embedded Facebook/Meta MIT license header |
| `react-dom.min.js` | `35f4f974f4b2bcd44da73963347f8952e341f83909e4498227d4e26b98f66f0d` | React DOM production UMD; embedded Facebook/Meta MIT license header |
| `react-flow.min.js` | `b3a79ccbda5d56b94a884ef46ed9990bf482afa3ff9651d8aacbb125509f4559` | React Flow UMD from the archived vendored distribution; MIT |
| `react-flow.css` | `4fb85a28b2a01dab75c9a89ad7de8f0b14169f05046ace4e2b66c2072f2a5c68` | React Flow distribution stylesheet; MIT |

The archive did not retain package metadata precise enough to prove an upstream React
Flow version, so this recovery deliberately identifies the immutable bytes and source
commit instead of guessing a version. The current adapter uses only pan, zoom, fit,
selection, and node-drag presentation mechanics. Archived parsing, inference, editing,
runtime state, evidence, and graph-authority code were not copied.
