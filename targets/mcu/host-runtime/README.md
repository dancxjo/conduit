# Shared MCU Host mechanisms

This no_std package holds mechanisms shared by physical MCU Host compositions.
It is not another Root, planner, scheduler, application, or authority issuer.

`seed::Seed` decodes the existing version-2 immutable construction carrier. The
platform supplies its independently owned bytes and actual Host/IMAGE/configuration
identities. Exact payload consumption and the existing `MemberBootstrap` checks
remain mandatory. The parsed material still requires current-Boot admission;
parsing does not manufacture a fresh invitation or membership receipt.

`transport::Link` preserves the existing bounded 4096-byte decoder, u16 big-endian
framing, 64-byte transfers, 30-second read deadlines and 2-second write deadlines.
The physical `Carrier` owns connection, IO and enforcement of the passed local
deadlines. USB supplies packet operations; UART can supply stream operations
without changing admission documents. The trailing empty USB packet also receives
the finite write deadline. Whole-document transaction deadlines remain with the
caller. Reset discards only the connection's partial decoder state, never an
admitted Play or retained member identity.

The RP2040 implementation retains its target-specific flash address, USB class,
BOOTSEL handling and timers. The shared code consumes none of those platform
resources and contains no GPIO or application behavior. An ESP32 composition can
supply the same existing protocol over its independently owned UART without
copying the membership or execution kernel.

`membership::SpawnAdmission` shares the existing authenticated spawn exchange
between physical compositions. It borrows the current Host advertisement and
independently supplied running IMAGE, checks bootstrap/provision Body and image
correspondence, signs the same invitation proof, and accepts only the pinned
owner's exact current-Boot receipt. Existing or retained membership refuses this
entrance before carrier effects; returning Parts use the separate canonical
return protocol. Receiving a credential does not create a Plan, Play or native
authority grant. The platform retains the acknowledged receipt and its digest.

`member_return::RetainedReturn` carries the existing signed image-projection
return protocol over the same framed interface. Original Body and Part identity,
current Boot, pinned owner, exact immutable image references and acknowledgement
remain checked; interruption never falls back to spawn. Native publication and
grant issuance stay with Root. A grant-copy callback runs after the image hash
vector is allocated, so Root measures actual remaining preparation headroom at
the same point as before. Images retain the existing 24KiB aggregate ceiling and
the canonical finite image-count bound. No decoded offer catalog is retained.
Owner-issued expiry remains in the exact signed challenge transcript; the
admitting owner checks its clock and proposal lifetime. MCU monotonic time is
not substituted for that independently scoped clock.
