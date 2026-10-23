# UART ingress at the first expected sampling time

The cadence-derived run admitted a fresh Boot of the same corrected image and
verified start-held and release. Near the first expected sampling time, the
control receiver returned `malformed` without advancing beyond command 16; the
last reply could not be parsed completely. No valid observation or attributed
quiescence was obtained. The raw bounded ingress and source-bound receipt remain
retained. A UART FIFO overrun during cooperative work is a working hypothesis,
not a verified sensor failure.

The next protocol explicitly grants at most 64 ingress bytes at a time, below
the FIFO extent, with whole-frame deadlines and authentication after complete
reception. Protocol 2 is announced before any frame is sent; the sender refuses
an older peer rather than flooding it. The physical cadence remains 60 seconds.
The corrected image still needs hardware acceptance.
