# Physical native assignment staging against independent firmware pins

The public `cargo xtask prove environment assignment-ingress-esp32` proof passes
on the flashed ESP32 image. Root receives the actual 7,111-byte transfer, borrows
its seven resident computation records, validates the canonical assignment and
current Host/Boot, checks independent layout/roles/cadence, and retains ordinary
owned runtime tables. Wrong Source, stale Boot and corrupted canonical digest
refuse before acceptance; a second valid submission refuses as already staged.
The complete initialized provider advertisement is unchanged by this sequence.

The final Boot is boot/008e7cd6850de760a5fc27951c75ff00, generation 1. Its retained
tables occupy 1,480 heap bytes. Root use rises from 81,084 to 82,564 bytes, with
peak 90,564 of 114,688. SDK peak is 33,716. The staging descriptor buffer is
retired; table ownership retains no wire payload borrow. No stack guard fired.
These are observations of these bounded cases, not a full runtime memory proof.

Independent provisioning predates the receiving Boot: Make emitted layout and
inventory from checked Source and the actual Root-backed Plan in capture
1792777335. Its 125 layout pins, selected contract/resource inventory and full
Source digest guard were provisioned through src/appliance_layout.rs before
flashing. Received packets never supply expected layout pins. A fresh Boot
7950 first passed; the final image adds the ready-reply deadline and passes again
in capture 8227. The zip retains all three complete captures. The host compiler
receipt records delivery=false before the separate physical ingress step; the
later ingress receipt establishes actual delivery and retained tables.

The shared builder now writes complete fields into admitted uninitialized
storage only after validation. The safe value API uses the same builder without
allocation. Its preparation-only boxed entry allocates one fixed-size object;
CPU controls prove its identity matches and its allocation is retired on layout
refusal and on cadence refusal after tables were constructed. Every unsafe write
has an exclusive aligned destination; failed partially initialized tables are
dropped before the parent remains inaccessible. Runtime memory and native owner
admission are still separate.

Stack measurements caught an oversized intermediate: a boxed-result wrapper
initially required 5,456 bytes. Field destinations plus separate construction
reduce the overlapping path to stage 640 bytes, admission wrapper 80, common
admission 32, and table builder 2,336. Construction finishing has its own
1,568-byte frame after table construction returns. The frame excerpts and ELF
hash are retained; this is not an exhaustive static stack bound.

The supported CPU proof passes seven Core controls, five legacy assignment
goldens, 28 Make controls, one generated-module Source/layout guard control and
one command-framing control. All three generated modules compile. Strict focused
Make and xtask Clippy, 19 32-bit execution/publication controls, formatting and
complete-delta whitespace checks pass. The optimized ESP32 build retains three
unused native issuance warnings. Both flashes completed; the final artifact hash
is in receipt.json.

Staging is not native possession, binding, reservation, Body/Play installation,
or complete runtime memory/work admission. Sensor/Face/Mask execution, positive
station/mDNS, recorder custody/transport and a physical overnight run remain
unfinished. No dev or stable acceptance is claimed for this appliance slice.
