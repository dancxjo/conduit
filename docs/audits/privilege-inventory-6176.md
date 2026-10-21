# Production graphical privilege inventory (#6176)

The [verified receipt](evidence/privilege-inventory-6176/verified.json) and
[retention index](evidence/privilege-inventory-6176/index.json) bind the normal
x86_64 `native-compositor` product to clean Source
`9983d3f8e3a842a723581631a93d329675804305`. One supported normal PROFILE build
produced kernel `9dae6197091f060e4dd35b2f11462e95c7349727e7556b3adcee3f7d0e1fec44`
and ISO `eb556954076f5bddc5c0ad691600d978d884ed21ca170b0b7854459a7b39d9c6`.
The subsequent evidence-retention commit changes documentation only; it does not
rename this compiled Source. Full verification requires checking out the stated
clean Source, not substituting the retention commit's identity.

The executable verifier matched the sealed manifest, actual kernel, exact
product-role build record, map name/hash, all 21 reviewed source responsibilities,
Root/embedded ELF load-segment observations and the complete locked target-specific
Cargo dependency/feature closure. The ten ELF observations include the Root;
image presence does not establish profile selection or a protected execution.
The current link map retains product controller, Face/Mask, compositor and
protected-service facades. The [architecture inventory](../architecture/conduitos-microkernel.md)
classifies their Root authority separately from the independently linked guests
and accounts for all nine current service profiles and the separate ordinary pool.

Eight focused contracts passed, including transitive-only dependency feature
substitution with an unchanged root line, stale source/kernel/role/map, dirty
source, malformed ELF and forged isolation/execution claims. Production xtask
Clippy with warnings denied, formatting and patch hygiene passed. The normal
native build passed with 21 existing native warnings; it was not a warnings-free
native lint run. No emulator was started for this audit.

An actual directory-replacement control paired the current exact inventory/map
with the earlier observed `e1623417` / `b36f75cf` build record. The supported
verifier refused with `privilege-inventory-invalid: dirty, replaced or wrong-role
build record`, exited 1 and wrote no success receipt. Original evidence was not
overwritten. The index retains file hashes and local raw artifact/log locations;
large ELF, ISO and map bytes are not duplicated into Git by this small receipt.

The contract is source/link inventory evidence, not an exhaustive computed call
graph, installed address-space proof, service-entry receipt, hardware or DMA
isolation, recovery proof, or completion of the microkernel parent #6175. Actual
service confinement and recovery keep their separately scoped evidence gates.
