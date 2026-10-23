# Bounded assignment Play reservation

The production ESP32 Root issuer accepts only the opaque validated assignment,
plus full Plan and fragment IDs whose canonical truncated hashes match its
sealed header. It checks the actual Root Host/Boot and shares its monotonic
reservation sequence with the rich fragment path. No rich Plan is reconstructed.

CPU controls run the actual issuer with named inert entropy/Boot providers.
Wrong full Plan, fragment, Host and Boot refuse without allocation. Invalid
requests preserve the previous reservation and counter; valid rich and bounded
reservations share a sequence, and superseded reservations cannot issue unused
native possession. Seven Root controls and strict Clippy pass.

ESP32 optimized release compilation passes with four unused-method warnings;
the new reservation method is intentionally not exposed by UART staging. No
image was flashed. A reservation neither authenticates a remote Body owner nor
installs a Body, issues native possession, admits complete kernel storage or
starts execution. Those gates remain required for appliance installation.
