# Selected digital contacts

Prerequisite [#6515](https://github.com/dancxjo/conduit/issues/6515) of the
[Mini PiTFT vertical](https://github.com/dancxjo/conduit/issues/6513).

The generic `machine/digital/access` Fore accepts observation or Boolean drive
requests and returns the actual observed level, or an exact refusal. Physical
pin, direction, safe level, mapping and electrical ownership are selected Host
facts. Requests cannot select a pin, change direction, configure pulls, or take
over another function. Independent contacts retain distinct capability, Base,
provider and resource identities; duplicate capability IDs refuse atomically.

The BCM2835 mechanism configures only one exclusively owned input pin, or sets
its latch before changing it to output. Existing output/alternate functions
refuse before writes. It preserves all unrelated register fields and uses the
correct level/set/clear bank. Revocation and drop leave an owned output driven
at its declared safe level. Unexpected function changes signal provider loss
and preserve the unknown owner's configuration. Re-admission requires explicit
Root ownership handoff, since floating an output is not its safe driven level.
The native register window must serialize function-select changes and enforce
ordered MMIO. Inputs depend on independently established electrical pulls.

`ReadyDigitalBase` retains real native possession before advertising a Back,
then consumes that ownership into an exact selected Plan/Play Host Call. The
existing Base capability table enforces authority, operation count and
revocation. Preparation retains all canonical storage; dispatch allocates
nothing. The operation factory accepts a finite unique inventory of at most64
contact capability IDs. These cooperative checks do not confine native code
sharing the same address space.

The optional ARMv6 `arch::minipitft` resource constructor owns the exact B+
attachment wiring and preserves it through native-provider teardown. It grants
no authority from board recognition and issues no ST7789 commands. Its unsafe
Root contract requires exclusive resources, electrical approval, ordered device
mappings, serialized pin-function changes and a fixed250MHz core clock in the
selected image profile. It is not invoked by the default board image.

Standing validation: `cargo xtask check device-protocols` includes
`device-protocols.digital`. Register models and canonical planning/dispatch
checks establish software contracts. They do not establish an electrical
waveform, visible display, real button behavior or physical boot.
