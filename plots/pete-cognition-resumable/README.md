# Resumable Pete cognition

The same cognition Gears use an episode State Back whose exact initializer is
selected before planning. The hosted application loads that initializer from
the Body and Wake checkpoint journal. The existing scheduler emits the initial
memory and advances it on each completed episode. The checkpoint Fore lets
the host persist each committed state using the synchronized file residence.

The checkpoint holds the completed episode count and at most four selected
observations and responses, within 4 KiB. Oldest entries are evicted as needed
to fit. Recovery checks the owner and count before any recalled history can enter
a model request. A replacement Play continues the episode count in the same Wake.

Run `cargo xtask check pete-cognition` for the proof. It commits the first Play's
checkpoint, reopens the journal, then selects the recovered bytes in a new
Host Boot and Plan. A fenced model verifies that the next episode recalls
the retained switch label. The application supplies the observation frames and
handles the checkpoint Fore. This proof covers graceful replacement and
synchronized checkpoint recovery within one Wake.
