# Recorded language revision inspector

Generate a standalone inspector from the actual flushed event trace:

```sh
python3 proof/evidence/language/revision_inspector.py \
  semantics/language/training/ewt_joint_v2/native_stream_planned_events.jsonl \
  /tmp/parser-revision-inspector.html
```

Open the HTML in a browser. Previous, Next, and the slider select an event;
Load another recorded JSONL replaces the trace locally. On browsers that support
the local File System Access API, Follow local event file observes newly flushed
complete JSONL lines once per second; Stop following retains the last observed
snapshot. The producer status remains unknown. Trailing incomplete lines are
ignored until flushed, and malformed complete records report an error without
replacing the last valid snapshot. The viewer makes no
network requests and never runs the parser. It displays the original elapsed
milliseconds, candidate ordinals, explicit agreement, independent frontiers,
identities, and full native bytes. The source-file digest identifies the evidence.

The retained v2 trace is a narrow four-token profile. It changes its provisional
analysis as context arrives; its stable and committed frontiers remain zero.
This recorded inspection is not a live session, real-time speech, physical
playback, or proof that the full linguistic-spine acceptance contract is met.
