# Recovered producer and binary provenance for PR6384

Source `a161489278d7170d309db9d35add9216111127d7`; historical preparation base `fdc2a582dd20c19d5f9479f66746e36e036a228d`. This supplement recovers the actual local producers/argv/rawlogs and historical expected Beam ELF hash. It adds no implementation or new execution.

All nine recorded mechanism/test source hashes equal current PR bytes. Complete base-to-PR delta shows every other tracked production dependency input unchanged; only evidence and supported-suite routing are additional. Historical WIP receipts did not capture a whole clean checkout, so this is source-byte correspondence, not relabeled clean a161 execution.

The actual Beam executable is now retained compressed with exact original byte count/hash, alongside its actual cargo command, rawlog and producer. Plot+Language strict library Clippy flags are recovered from the checked producer/receipt and joined to the exact previously published log; this is --lib, not all-target lint.

Generic current-generated/no_std behavior is proved by the successful outer test's checked compiler/inner-execution/no_std assertions. Four inner binaries were temporary and deleted by the existing helper. The older generic binary predates the final generator hash and remains explicitly historical; it is never relabeled as final.

No redundant cold build, test campaign, CI observation or owner-cache change was performed. Exact-head Candidate and accepted Integration/publication remain required; no6065/4907 closure or controller/Current authority is claimed.
