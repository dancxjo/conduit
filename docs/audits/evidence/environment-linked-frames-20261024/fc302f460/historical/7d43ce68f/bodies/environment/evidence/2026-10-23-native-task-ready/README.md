# Actual ESP32 task custody

The supported `cargo xtask prove environment owner-prepare-esp32` workflow
completed physical preparation on the ESP32-WROOM, retaining the real host Body
owner through signed reservation and native preparation. The target task owned
the 4,640-byte execution context and acknowledged `ready-held`; its measured
held future is 3,536 bytes. Both native providers were issued and idle, with
zero Steps, submissions, completions and publications.

Root heap use was 105,164 of 114,688 bytes, with a 109,704-byte peak. The exact
image and source identity are retained beside this note. This establishes task
custody after the planner/literal stack fixes; it does not establish attributed
Body start, release, climate sampling, Face output or overnight recording. The
host proof process exits after preparation; persistent owner custody is still
required before release.
