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
bytes to the independent queue. TX and RX ordinals are independent. After
acknowledgement it takes the next queued TX before waiting for RX. Pressure on
that sequential take routes one RX; pressure on receive delivery and other
malformed/terminal observations end the finite cycle. It does not retry an empty
queue, acknowledge a partial write, or emit a fabricated frame. The containing
Root stops and retires the admitted transfers when its separately approved
bounded TCP exchange produces the actual completion receipt. One USB read is one frame in this pinned profile; split or
coalesced frame reconstruction and VLAN/jumbo frames are unsupported.

`cdc-ecm-profile.json` describes ordinary Source specializations and bounded
operation counts. Assemble imports from service/frames once, followed by
`cdc-ecm-types.conduit`, `cdc-ecm-profile.conduit`, then the two bodies. The start
Value supplies a typed USB write request containing empty bytes because Source
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

The entry takes one bounded Value seed through the existing typed singleton.
The terminal flow uses the existing typed finite concat; a stopped state routes
to stopped again, so its write selection closes the concat’s right side empty.
These adapters advertise the exact external contracts instead of projecting a
bounded Fore directly onto a pure-expression Back without that outer contract.
The terminal envelope is 512 bytes: the checked canonical encodings of all
terminal variants reach 416 bytes, so the earlier 256-byte annotation was too
small. The seed envelope remains 4096 bytes. Neither adapter admits USB or
network authority.

The full typed ECM event transcript can exceed the observer default of 256 KiB.
For this bounded TCP proof, admit `--maximum-transcript-bytes 2097152` through
`cargo xtask make conduitos protocol-run`; each transcript remains bounded and
the receipt retains that admission. This changes observation storage, not USB,
network, Plan or Play bounds.
