# Single pending-IN detach observation

Both x86_64 and AArch64 product kernels at1f5 run the corrected checked CDC package e245 with Source08cb. Each external device retains the first requested64-byte IN after one exact12-byte transmit, then sends an actual USBredir disconnect. The wire gate requires that original IN packet ID is cancelled exactly once and its matching cancellation response contains zero bytes and cancellation status1. No further class controls or bulk requests are allowed by the exact sequence gate.

Canonical Core decoding validates precisely two ordered typed outputs: transmitted(requested12/actual12/ordinal0), then provider-lost(empty). Both actual Plays retire complete without protocol refusal. These are single detach observations, not repeated-disconnect, late admitted Native completion, physical hardware or full cancellation acceptance.

Original runner receipts remain diagnostic-only and unchanged. Separate post-observation validations bind original receipt hashes and the exact later decoder/checker hashes, so a later gate is not falsely attributed to the original run. Source/kernel/package identity and full transcripts remain in those original receipts. The decoder uses the existing canonical Core API, compiled read-only against a retained library; no product change or Cargo build was performed for this gate.
