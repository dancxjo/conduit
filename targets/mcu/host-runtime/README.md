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
