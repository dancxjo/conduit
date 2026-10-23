# Source bootstrap physical startup refusal

The image at revision `060f65194` compiles and flashes, but fails physical startup.
A normal-reset espflash monitor decoded a ProCpu stack-guard panic while the
Source constructor decoded a structured type inside a portable expression.
Boxing retained software reduced the Root entry frame from 8,784 to 4,528 bytes
and increased available stack to 31,556 bytes, but did not resolve the failure.
The constructor frame remains 11,200 bytes; recursive expression nodes add
1,216 bytes per level and type decoding adds 384 bytes per level.

The read-only software-ready proof failed waiting for the first Root offer.
This is negative HIL evidence, not software availability, climate sampling,
Body installation, or a completed deployment. The previously verified fixed
memory image was restored successfully. A fresh physical negative-network fixture passed:
one association attempt failed, and the AP was restored in the same Boot.
The retained restored result and offers prove that recovery; they do not prove
station success or climate sampling.

ELFs remain under the thread outputs directory. The receipt records their
hashes. No credentials were used in the startup observation.
