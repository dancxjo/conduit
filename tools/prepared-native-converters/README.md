This Make-time tool adds scoped prepared converters to an independently approved,
completed generated binding. It does not check Source or confer admission authority.
The input manifest and resulting whole output manifest must be independently pinned.
Normal checked generation remains the default.

Run with `cargo +stable run --manifest-path tools/prepared-native-converters/Cargo.toml -- ORIGINAL_OUT_DIR ORIGINAL_SELECTED_INPUT_RECEIPT NEW_OUT_DIR MANIFEST EXPECTED_HEADER_SHA256`.
The output directory must not exist. The tool assumes Default Rust binding layout
and refuses any ordinary converter mismatch. It parses Rust with `syn`, uses parser
byte spans, and preserves every original byte by insertion only. It copies and hashes
every output file, retaining all original descriptors, contracts, ordered laws,
binding layouts, and Source programs. Only the header gains methods. The manifest
records exact source inputs, output extents/hashes, generator inputs, and each edit.

Runtime Rust changes to `semantics/language/src/lib.rs` and
`parser_session_execution.rs` after completed generation are recorded separately;
normal Cargo compilation of those current files is still required. Every other
original selected generation input must remain unchanged. This tool emits no fresh
Source-check or public Session acceptance claim.
