# Todo checkpoint durability proof

Development evidence for #5265. Stable acceptance must be recorded after the
candidate, integration and publisher gates complete; this document does not
claim that an unmerged follow-up is accepted.

Run `cargo xtask check todo-durability` to exercise the selected checkpoint
resource, public Body write/read Plays and retained Owner lifecycle.

The executable hosted continuity fixture is
`products/conduit/src/body_owner/todo_continuity_tests.rs`:
`same_body_three_items_recover_on_another_admitted_host_without_a_cache`.
It births one Body, adds Milk/Eggs/Bread, completes Milk, publishes and lulls,
then resumes on a second explicitly admitted Host and fresh Boot. The handoff
contains biography and lineage, without Todo state bytes. The original Owner
directory is deleted before the second Host reads the selected resource.
The recovered state digest is
`sha256:11ac0e37be11c298a5654b68270aca7290d4d24e6612a11ed588507e7a3a31be`.
The supported suite emits `TODO_CONTINUITY_RECEIPT=` with correlated source,
Body, selected content generation, write/read Plans, Plays and terminal Signs.

The resource tests exercise duplicate publication refusal, selector-write
failure with an unchanged prior publication, exact retry, stale generations,
corrupt bytes, incompatible envelope schema, truncated records, unpublished
partial candidates and provider-profile refusal before storage mutation.
The public Body read tests additionally assert that failed reads emit no state
Fore, and require explicit migration or a fresh list choice for old namespaces.
Owner tests reject foreign grants, corrupt retained write receipts and missing
read Signs, and retry only an explicitly selected published version.

Cancellation is tested by
`cancelled_wait_has_no_command_effect_or_committed_delivery`: the terminal Sign
correlates the cancelled Play, no checkpoint or state Fore appears, and a later
command is rejected. The shared kernel's
`cancellation_rejects_late_host_completion_and_releases_pending_input` tests the
Host Call completion boundary separately; the checkpoint Back uses that kernel.

This proof selects a shared local filesystem residence and performs an explicit
fixture handoff. A successful write acknowledges candidate and selector fsync;
it does not prove physical power-loss survival or remote transport. The crash
cases inject partial or unpublished file state deterministically. Rendered
three-Mask encounters and deployed journey acceptance belong to #5300/#5267/
#5268 and are not credited by this persistence proof.
