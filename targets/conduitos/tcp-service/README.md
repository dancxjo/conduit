# Canonical TCP service source prerequisite

This standalone x86_64-none image retains smoltcp 0.13.1 Interface and exactly one
TCP socket. The configuration matches production's IPv4/Ethernet, one address,
one route, four neighbors and 4096-byte RX/TX socket buffers. `alloc` supplies
owned buffers/SocketSet storage; there is no forged static borrow. Allocations
occur during initialization only. No socket, NIC, device pointer, DMA, IRQ,
clock, deadline, endpoint-discovery or reconnect authority is granted.

## Memory and build contract

Binary `conduitos-tcp-service`, manifest `tcp-service/Cargo.toml`, dependency
`conduitos-tcp-core` (only dependency smoltcp 0.13.1). Guest remains a separate
workspace. RX/RO image 0x400000..0x600000; exchange 0x601000 (4096 bytes);
transfer 0x603000..0x607000 (16384 bytes); private stack
0x608000..0x708000 (1MiB); metadata 0x709000..0x809000 (1MiB).
Only first metadata page admits initialized RW/NX .data/.bss, never writable
image. Retained State is at metadata+4096; allocator control/bitmap at +8192/
+12288, heap from +20480. Fixed metadata is a finite engineering budget, not
measured heap/image/stack/runtime evidence. Build with existing freestanding
service flags and loader, entry `tcp_entry`, no SIMD/FPU authority.

## Frozen Root proxy API

`wire::{BYTES=64,Envelope{instance,sequence},Request,Response,
encode_request,decode_request,encode_response,decode_response}`. Encode accepts
an output slice >=64; decode requires exactly64. Magic CTCP/version1, header
32 bytes matching the existing instance/request convention, eight LE u32 args.
Unused bytes must be zero. Envelope IDs nonzero; installed guest admits the
instance once and requires consecutive request sequence from1.

Initialize transfer is exactly40 meaningful bytes:
seed u64@0, MAC6@8, prefix u8@14, reservedzero@15, localIPv4@16,
gatewayIPv4@20, remoteIPv4@24, localPort u16@28, remotePort u16@30,
initialMillis u64@32. `state::Initialize::encode` writes this layout. Root
retains the reviewed values; guest never selects/replaces endpoint or route.
Init -> Ready exactly once; time is Root supplied and nondecreasing <=i64::MAX.

Requests:
- Poll{millis,ingress}: optional transfer-prefix Ethernet frame0..1514 bytes.
  No previous outgoing frame may remain. Calls canonical poll_maintenance,
  poll_ingress_single ONCE, then poll_egress ONCE, never unbounded Interface::poll.
  Adapter reserves output credit before exposing incoming token and returns
  None for exhausted transmit credit. Fixed four-frame queue; no frame dropped
  after successful token consumption.
- Status -> Status{state,receive,send,frames,next_frame,polls,flags}.
  Poll returns this same status. receive=recv_queue, send=send_queue,
  polls cumulative checked u32. State numeric mapping Closed0,Listen1,SynSent2,
  SynReceived3,Established4,FinWait1=5,FinWait2=6,CloseWait7,Closing8,LastAck9,
  TimeWait10. Flags bit0 can_send, bit1 may_send, bit2 may_recv, bit3 is_active.
- TakeFrame{frame} copies exact head frame to transfer -> Frame{frame,bytes}.
  IDs globally consecutive from1. Take exactly once; then Root validates the
  frame against its egress authority filter and physically sends it. Only after
  success AckFrame{frame} -> Ready removes/clears head. Drain via frame+1 for
  the original finite frames count; then nextPoll. Send/validation failure
  quarantines without Ack or retry. Status.next_frame is0 iff frames0.
- Write{bytes} positive <=16384 transfer bytes, only after status.can_send;
  Written{bytes} is canonical send_slice partial acceptance1..4096.
- Read{maximum} positive <=16384, only after receive>0 or EOF status;
  Read{bytes} transfers canonical recv_slice bytes0..4096. Zero means canonical
  !may_recv||!active with no queued data, not a pending-read response. Root must
  poll again for a live empty receive queue, never repeatedly issue Read.
- Close -> Ready invokes canonical socket.close; keep polling/draining until
  Closed, FinWait2 or TimeWait, then Abort -> Ready and retire lease, matching
  current close_tcp_with_device behavior. Abort invokes socket.abort and never
  creates another socket; Root retires the whole lease.

Refusal enum: Bounds1,Malformed2,Order3,Capacity4,Connect5,Send6,Receive7,
Closed8,Clock9. Every Refused/protocol fault/transport failure is terminal to
Root; State is not a recoverable retry interface. Init/write/read/close are
not hidden polling loops. Empty caller Read/Write remains a local Root Ok(0).

Root proxy must preserve connect's maxpoll bound, Established check and inactive
refusal; read's deadline-vs-clockless polling behavior, queued-data-before-EOF;
write's finite poll limit and partial count; flush's send_queue==0 condition;
close's retirement states. Root supplies the existing deadline.elapsed_millis
or clockless polls/100 fallback and checks cumulative u32 and operation budgets.
One bounded API pass may replace multiple legacy full-poll iterations, so
protocol poll counts/engineering bounds are not timing-equivalence evidence.
Root owns and returns the SAME VirtioNetReady; no guest teardown can own DMA.

## Authority and limits

Root MUST broker output Ethernet frames against the admitted MAC/localIPv4/
localPort/remoteIPv4/remotePort. ARP is limited to Root-approved gateway/peer.
Guest-generated frame, status, acknowledgments, sequence IDs or send_queue0
is not endpoint authority or physical delivery evidence. Root can check exact
finite lengths/counts, monotonic time, correlation, and packet endpoint policy;
canonical TCP state/ACK validity cannot be independently certified without
another TCP stack. The semantic TCP result is delegated guest state, not a Root
security fact. No device/DMA isolation, runtime operation proof, production
wiring, measured stack/heap fit, build/test or acceptance claim is made here.
