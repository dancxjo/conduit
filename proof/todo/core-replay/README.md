# Installed Todo core replay

Issue [#5264](https://github.com/dancxjo/conduit/issues/5264) requires a
reproducible add-three, complete, reopen and remove sequence with exact state
digests. The [authored Source](../../../plots/todo/live.conduit) and its
application semantics are unchanged. The
[installed replay tests](../../../targets/std/src/installed_std/body_kernel/todo_core_replay_tests.rs)
execute that Source through the checked expansion, actual std offer, sealed
Body Plan and production kernel, using ordinary typed external Fore boundaries.

Run `cargo xtask check todo-state`. The kernel stage prints the complete
`TODO_CORE_REPLAY_RECEIPT` and both `TODO_CORE_REFUSAL_RECEIPT` records. The
two positive runs use fresh Hosts, Boots and independent fixture Bodies. Each
state is compared with explicit expected fields, round-tripped through the
owning bounded codec, and compared byte-for-byte across the runs. This is replay
conformance, not one Body migrating between Hosts; #5265 owns continuity.

The initial state is the checked empty Groceries list, revision 0, next ID 1.
The state Info Kind is `conduit.todo/state@1` (at most 1,635 bytes); commands
use `conduit.todo/command@1` (at most 75 bytes). IDs are allocated by the
application meaning, not displayed ordinals. List identity belongs to its Body.

| Command | Revision | Next ID | Ordered items |
| --- | --- | --- | --- |
| Add Milk | 1 | 2 | task-1 Milk, open |
| Add Eggs | 2 | 3 | task-1 Milk, task-2 Eggs, both open |
| Add Bread | 3 | 4 | task-1 Milk, task-2 Eggs, task-3 Bread, all open |
| Complete task-1 | 4 | 4 | Milk completed; Eggs and Bread open |
| Complete task-1 again | 4 | 4 | Exactly the same state bytes |
| Reopen task-1 | 5 | 4 | All three open |
| Remove task-2 | 6 | 4 | task-1 Milk, task-3 Bread, both open |

[Replay receipt](replay.json) retains the exact source hash, Source/checked
Plot identities, command and state digests, each selected Plan/Play and terminal
Sign. Child invocation count and parent Play correlation are asserted. The
selected pure child has no Host Calls, resource bindings or authority grants;
the Host's other available offers are not selected effects.

[Refusal receipts](refusals.json) retain actual failed remove and completion
of an absent item after the three successful adds. The failed command emits no
state. Previously delivered states are not reclassified as a successful terminal
Play. Empty text is refused before a command can be encoded. Existing core
conformance separately covers capacity, malformed state, identity/revision
exhaustion and removed-ID nonreuse.

These are hosted executable conformance records. They establish neither durable
publication, rendered Masks, actual browser UI, device output nor human
listening. Required Candidate, automated integration and identical-tree stable
publication remain separate acceptance gates before this issue closes.
