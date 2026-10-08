# Unicode IPA public authoring proof

The source slice preserves original notation/Unicode/partition/admission proofs from
4ae59f6985b2248fceb2dccf0d903aa1b0b03771 and inventory binding proof from
282862adab3bfc0ab115fa0b45e0278b44e05ba5. It was prepared against d932da60c7f04bde62c797126e298e38aa9819d0 and integrated with consolidated foundation b1c5a2b281e07d97b621057f3c504d808f14a3ce before final gates.

The receipt hashes the exact dirty-tree source bytes tested. Source was frozen throughout final gates; later source commit is byte-equivalent, and the subsequent proof commit changes evidence only. Generated build outputs and shared target artifacts are not committed.

`final-std.log` includes seven real ordinary Source/standard Back/HostCall constructor journeys. These cover inventory-independent phonetic Unicode, distinct phonemic membership, exact inventory/variety/revision, original byte/scalar spans, explicit alias provenance, syllable-boundary barriers, unit/text overflow, Native serialization, CST and highlighter preservation. Original four extracted proof targets remain in the full Speech test gate.

`final-no-std.log` checks the Speech crate without default features. This does not claim Std Host execution in no_std. No public Source formatter API exists; lossless CST, quoted-text serialization and syntax highlighting are the supported tooling boundary. Cargo formatting is not a Source formatting proof.

Finite input/profile/output and prepared-owner limits do not claim a measured whole-process heap ceiling or allocation-free parsing. Parsing/admission occurs during bounded Back preparation; runtime executes the actual HostCall and checks exact placement/startup digest before returning the prepared Native outcome.

Selected original failed probes are retained verbatim: nested full inventory hit the existing Type-depth bound (fixed by a separate owner); generated Native convenience constructors exposed flattened payload APIs; unsupported multiline startup expression and an unsuitable one-input runner were replaced with existing ordinary Source syntax and zero-input external Plot execution. Failures are development evidence, not passing gates.

Full Speech 317 and Std 665 tests, no_std and public catalog checks passed before a three-expression clone-on-Copy correction. Strict Clippy and seven actual constructor journeys passed on final source. The two hash sets and failed lint diagnostic are retained explicitly; no full-suite result is mislabeled as testing different bytes.
