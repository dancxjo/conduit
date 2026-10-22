# Item-order correction checks

Exact clean source `1ea0d49b8e2c4e36de0567e8dad27b00fb64291a` moves the unchanged `verify_isolation` helper before the unchanged tests. The targeted existing isolation test passed (one test); strict all-target xtask Clippy and workspace formatting both passed. The earlier lint-negative and catalogue checks remain unchanged in the parent packet, bound to their original source `52f76743743d72ca5a54bdb37feb4349ace759a0`. They were not relabelled as newly executed on this source.

See `checks.json` for exact commands, original paths and byte custody. Logs are lossless gzip originals. No browser campaign, QEMU, CI dispatch, PR update or other reported Candidate repair is represented here.
