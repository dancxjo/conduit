# Native worker terminal custody

The single-executor worker deposits its terminal execution object in Root's
private slot, marks the task inactive and returns without another await. Because
management runs on that same executor, it cannot inspect inactive state until
the worker poll has returned. The executor's static task-pool bytes remain
reserved and reported; task exit does not claim to reclaim static memory.

An attributed Quiesced acknowledgement requires inactive task state, successful
cleanup, revoked pulse/timer possession and zero in-flight work. Authenticated
Retire independently checks the terminal inactive state before taking custody,
returns the actual providers to Root, and drops the terminal kernel. Failed
cleanup retains custody and cannot acknowledge retirement. Neither task exit nor
a diagnostic descriptor creates provider possession or another Play.

The StaticCell slot remains one-use. A subsequent Play still needs explicit
fresh provider/resource generations, software preparation, reservation and
exclusive slot reinstallation. This change removes the forever-pending terminal
task; it does not implement rearming, owner-loss recovery or continuous recording.
The supported physical entrance is `cargo xtask prove environment owner-run-esp32`.
The ESP32 image builds; physical inactive-task/retirement acceptance is pending.
Refs #6337 and #6338.
