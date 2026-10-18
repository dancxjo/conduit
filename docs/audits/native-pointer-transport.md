# Native pointer transport prerequisite

The normal native product currently delivers keyboard actions. Completing
[#5989](https://github.com/dancxjo/conduit/issues/5989) also requires actual
pointer delivery through its current checked Mask and Source owner. This
transport slice does not complete that semantic entrance or its click proof.
[The Boolean slice](https://github.com/dancxjo/conduit/pull/5998) retains its own
source-pinned keyboard/Mask evidence.

One xHCI owner consumes the hardware event ring. A fixed inline inbox retains
at most sixteen completions, matching the existing hardware-ring extent;
preparation reserves it with the controller before device publication or
Source Play. Endpoint polling examines that finite inbox and at most one new
hardware event. Foreign completions remain ordered for their original owner.
An absent report is idle. Full retention refuses before consuming another
hardware event. A matching endpoint's wrong TRB reaches its exact validator;
it cannot be skipped to find a more convenient later completion.

Keyboard, pointer, control and FTDI waits select their own endpoint. Ordinary command
waits preserve other endpoints' transfers. Original retirement retains its
exclusive-owner contract and refuses foreign transfers rather than skipping
them to find a later acknowledgment. Unexpected command completions during an
endpoint receive refuse immediately; they cannot acknowledge a future command. Exclusive native
transport owners drain remaining retained events in their hardware order. This changes no
possession, authority, semantic dispatch or scheduler: only the original
controller owner can advance the physical ring.

Pointer begin/poll retains one exact pending transfer, attachment epoch and
normalized sample state. A second begin, stale attachment, malformed completion
or retention pressure cannot replace that pending transfer. The exclusive
receiver uses the same machinery and its existing finite wait bound. No retry,
heap growth or semantic callback is introduced by the transport.

Synthetic controller/DMA tests establish conservation, exact completion and
refusal behavior. They do not establish emulated or physical device execution.
Installed pointer acceptance still needs a normal Host product, actual QMP
motion/button events, current Show-bound typed interactions, owner effects and
fresh acknowledged Shows. Mask-local cursor/gesture work belongs in the next
slice. Source remains the sole owner of selection, and the existing graphical
limits remain 48 commands, 192 text bytes and eight retained surfaces.
