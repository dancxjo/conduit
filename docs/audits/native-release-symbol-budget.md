# Native release symbol budget

The documented Linux release entrance refused the clean c5d9600ff2faacbd9503217ac3849c6c4b5e338f x86-64 executable: the file was about 69 MiB against the existing 64 MiB native release-file limit, while its executable/data sections totaled about 54 MiB. The unstripped failed bundle is retained separately. ARM64 was about 63 MiB.

Set the ordinary Cargo release profile to strip symbols. This uses Cargo/Rust's standard final-artifact transformation, preserves the executable and required dynamic interfaces, and keeps the existing packaging/admission ceiling. It does not increase the byte limit or require a caller-only environment override. Debug builds retain their normal symbols.

Cargo accepts the profile configuration under locked metadata admission. A matching full Linux release with the same explicit strip setting sealed both x86-64 and ARM64 bundles. The x86-64 executable is 56,248,112 bytes, below the unchanged 64 MiB limit. Its installed owner completed one actual browser Todo add through the public first-add capture; that partial capture does not claim the entire Todo journey. Validation of this default profile at its own clean Source remains pending. Parent #5520's broader proven delivery requirements remain open.
