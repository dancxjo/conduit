# Lifecycle-control stack refusal

The first new control image booted, completed ambient admission and staged its
exact assignment, but failed native preparation before start/release. A second
attended diagnostic reset the same just-flashed image without rewriting it,
admitted the fresh Boot, and retained the UART trace. The hardware stack guard
fired while `native_kernel::prepare` was called through the new outlined
`native_owner_wire::receive` future. Neither run established a Body start or
physical sensor execution.

The linked receive frame is 3,760 bytes. Its caller now uses 2,624 bytes; the
prior successful held-worker image used a 4,064-byte caller with receive inlined.
The new image also has 136 fewer bytes of CPU stack span. The literal and kernel
preparation frames retain their earlier 432- and 2,256-byte sizes. These linked
measurements and the captured panic motivate moving cold request authentication
and reply framing out of the asynchronous preparation path. No guard or quantity
semantics were weakened. A corrected image still needs physical acceptance.

The raw ingress ring is bounded to the last 64 KiB. The receipt binds its carrier
and host executable by digest; the image receipt separately identifies the
unchanged flashed ELF. No private owner seed or network credentials enter these
artifacts. The retained JSON reports the incomplete result explicitly.

After outlining the cold authentication/encoding phases, the target build passed
and the enclosing control frame is 4,224 bytes, with receive inlined again;
preparation no longer pays the additional 3,760-byte nested frame. The CPU stack
span is 28,916 bytes. `frames-after.json` binds that measurement to the new ELF.
This is a linked resource improvement; the next physical run remains required.
