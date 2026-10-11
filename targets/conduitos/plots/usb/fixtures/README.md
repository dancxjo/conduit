This is an external bounded **USB device** fixture for the checked ECM Source,
not a Conduit Host Back, class driver, network stack, or proof receipt. It uses
the same usbredir protocol framing as the retained CDC-ACM device fixture.
A newly created Unix socket and transcript are owned for at most 120 seconds;
no libusb, personal device, image, or filesystem disk is opened.
The optional network mode owns a QEMU user-network process and a private
socketpair; it can reach the explicitly selected proof server through QEMU’s
ordinary gateway. Use a dedicated local proof server, never personal services.

The pinned 80-byte ECM configuration includes CDC Header/Union/Ethernet
functional descriptors, MAC string 4 (`020000000001`), communication interface
0 and data interface 1 with alternate 0/1, bulk IN81/OUT02 max64, MTU1514.
IRQ83 is recognized and unconsumed; notifications and dynamic link state are
unsupported. The Source admits the exact descriptor and MAC string, explicitly
sets configuration, filter `0x000f` and alternate 1. Interface-up is distinct
from network permission and actual network service-up.

The Source consumes at most sixteen genuine frames from the independently
admitted frame queue, sends their exact bytes, sends an actual ZLP when length
is a multiple of64, acknowledges the exact queue ordinal only after full OUT,
reads at most1514, validates metadata/header/bounds, and submits identical RX
bytes to the independent queue. Pressure or any malformed/terminal observation
ends the finite cycle; it does not retry, acknowledge a partial write, or emit a
fabricated frame. One USB read is one frame in this pinned profile; split or
coalesced frame reconstruction and VLAN/jumbo frames are unsupported.

`cdc-ecm-profile.json` describes ordinary Source specializations and bounded
operation counts. Assemble imports from service/frames once, followed by
`cdc-ecm-types.conduit`, `cdc-ecm-profile.conduit`, then the two bodies. The start
record supplies a typed USB write request containing empty bytes because Source
has no byte literal constructor; the Source checks its length is zero before
any effect. This input contains no host addresses or authority.

The integration owner must admit network authority independently, select the
same configured MAC for the retained smoltcp Device, install the queue Host
Calls, and interleave the existing bounded network poll adapter so queue
pressure remains explicit. DMA/device/endpoint generations, Plan/Play identity,
actual native cancellation and emulator exchange are integration proof duties.
This candidate alone does not establish #5255 completion.

Internal fixture validation:
`python3 -m unittest discover -s targets/conduitos/plots/usb/fixtures -p test_cdc_ecm_device.py`.
Run the device with a fresh `--socket`, fresh `--transcript`, and explicit
`--reply-hex` containing one distinct Ethernet II frame (14..1514 bytes).
QEMU must be connected via the ordinary protocol-run external usbredir option;
root port/controller selection remains in the separately admitted Root.

For actual network replies, select `--network-log` instead of `--reply-hex`.
The device forwards each completed Ethernet frame unchanged through an owned
QEMU socket/hub/user-network backend. It retains at most eight queued receive
frames and eight transmit frame envelopes, checks lengths before allocation,
and records both Ethernet directions alongside the USB transcript. It never
constructs TCP packets or returns a fixed reply in this mode. Backend exit,
queue exhaustion, malformed frames and deadline expiry refuse the run. The
owned process is reaped at exit. This mode alone still proves neither a guest
network exchange nor #5255 acceptance.
